use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{bail, Context as _};
use async_trait::async_trait;
use jsonrpsee::{
    core::client::{ClientT as _, Error as JsonRpcError},
    http_client::HttpClientBuilder,
    rpc_params,
};
use tracing::warn;

use crate::{
    config::TxSenderConfig,
    docker::DockerEnv,
    log_provider::LogPathProvider,
    test_case::watch_log_for_panics,
    traits::{NodeT, SpawnOutput},
    Result,
};

pub struct TxSender {
    spawn_output: SpawnOutput,
    pub config: TxSenderConfig,
    client: String,
}

impl TxSender {
    pub async fn new(
        config: &TxSenderConfig,
        docker: Arc<Option<DockerEnv>>,
        failure_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<Self> {
        setup_tx_sender_database(config, docker.as_ref().as_ref()).await?;
        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;
        let client = config.local_url();
        watch_log_for_panics(config.log_path(), config.label(), failure_tx);

        let tx_sender = Self {
            spawn_output,
            config: config.clone(),
            client,
        };
        tx_sender.wait_for_ready(None).await?;
        Ok(tx_sender)
    }
}

#[async_trait]
impl NodeT for TxSender {
    type Config = TxSenderConfig;
    type Client = String;

    async fn spawn(config: &Self::Config, docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        match docker.as_ref() {
            Some(docker) => docker.spawn(config.into()).await,
            _ => bail!("tx-sender currently requires Docker"),
        }
    }

    fn spawn_output(&mut self) -> &mut SpawnOutput {
        &mut self.spawn_output
    }

    fn config_mut(&mut self) -> &mut Self::Config {
        &mut self.config
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }

    async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<()> {
        wait_for_tx_sender_ready(&self.client, timeout).await
    }

    fn client(&self) -> &Self::Client {
        &self.client
    }
}

async fn setup_tx_sender_database(
    config: &TxSenderConfig,
    docker: Option<&DockerEnv>,
) -> Result<()> {
    let docker = docker.context("tx-sender database setup requires Docker")?;

    let env = vec![format!("PGPASSWORD={}", config.db_password)];
    let cmd = vec![
        "psql".to_string(),
        "-h".to_string(),
        "127.0.0.1".to_string(),
        "-p".to_string(),
        config.db_port.to_string(),
        "-U".to_string(),
        config.db_user.clone(),
        "-d".to_string(),
        "postgres".to_string(),
        "-v".to_string(),
        "ON_ERROR_STOP=1".to_string(),
        "-c".to_string(),
        format!(
            "CREATE DATABASE {} OWNER {}",
            sql_ident(&config.db_name),
            sql_ident(&config.db_user)
        ),
    ];

    let timeout = Duration::from_secs(30);
    let start = Instant::now();
    let mut last_err = None;

    while start.elapsed() < timeout {
        match docker
            .exec_in_named_container("postgres", env.clone(), cmd.clone())
            .await
        {
            Ok(()) => return Ok(()),
            Err(e) => {
                warn!(
                    "Failed to create tx-sender database {}, retrying: {e:#}",
                    config.db_name
                );
                last_err = Some(e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    Err(last_err.unwrap()).with_context(|| {
        format!(
            "Failed to create tx-sender database {} after {timeout:?}",
            config.db_name
        )
    })
}

fn sql_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

async fn wait_for_tx_sender_ready(endpoint: &str, timeout: Option<Duration>) -> Result<()> {
    let timeout = timeout.unwrap_or(Duration::from_secs(30));
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        let client = HttpClientBuilder::default().build(endpoint)?;
        match client.request::<u64, _>("send_tx", rpc_params![]).await {
            Err(JsonRpcError::Call(_)) => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }

    bail!("tx-sender failed to become JSON-RPC ready within the specified timeout")
}
