use std::{
    fs::File,
    process::Stdio,
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
use tokio::process::Command;
use tracing::{info, warn};

use crate::{
    config::TxSenderConfig,
    docker::DockerEnv,
    log_provider::LogPathProvider,
    test_case::watch_log_for_panics,
    traits::{NodeT, Restart, SpawnOutput},
    Result,
};

pub struct TxSender {
    spawn_output: SpawnOutput,
    pub config: TxSenderConfig,
    client: String,
    docker: Arc<Option<DockerEnv>>,
    failure_tx: tokio::sync::mpsc::UnboundedSender<String>,
}

impl TxSender {
    pub async fn new(
        config: &TxSenderConfig,
        docker: Arc<Option<DockerEnv>>,
        postgres_container_id: &str,
        failure_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<Self> {
        let docker_env = docker
            .as_ref()
            .as_ref()
            .context("tx-sender requires a Docker environment for its database setup")?;
        setup_tx_sender_database(config, docker_env, postgres_container_id).await?;

        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;
        let client = config.local_url();
        watch_log_for_panics(config.log_path(), config.label(), failure_tx.clone());

        let tx_sender = Self {
            spawn_output,
            config: config.clone(),
            client,
            docker,
            failure_tx,
        };
        tx_sender.wait_for_ready(None).await?;
        Ok(tx_sender)
    }

    fn spawn_local(config: &TxSenderConfig) -> Result<SpawnOutput> {
        let bin = std::env::var("TX_SENDER_E2E_TEST_BINARY")
            .map(std::path::PathBuf::from)
            .map_err(|_| {
                anyhow::anyhow!(
                    "TX_SENDER_E2E_TEST_BINARY is not set. Cannot resolve tx-sender binary path"
                )
            })?;

        let stdout_path = config.log_path();
        let stdout_file = File::create(&stdout_path).context("Failed to create stdout file")?;
        info!(
            "tx-sender {} stdout logs available at : {}",
            config.alias(),
            stdout_path.display()
        );

        let stderr_path = config.stderr_path();
        let stderr_file = File::create(stderr_path).context("Failed to create stderr file")?;

        Command::new(bin)
            .kill_on_drop(true)
            .envs(config.env())
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .spawn()
            .context("Failed to spawn tx-sender process")
            .map(SpawnOutput::Child)
    }
}

#[async_trait]
impl NodeT for TxSender {
    type Config = TxSenderConfig;
    type Client = String;

    async fn spawn(config: &Self::Config, docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        match docker.as_ref() {
            Some(docker) if docker.tx_sender() => docker.spawn(config.into()).await,
            _ => Self::spawn_local(config),
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

#[async_trait]
impl Restart for TxSender {
    async fn wait_until_stopped(&mut self) -> Result<()> {
        self.stop().await?;

        match &self.spawn_output {
            SpawnOutput::Child(_) => { /* local process already killed by stop() */ }
            SpawnOutput::Container(output) => {
                let Some(env) = self.docker.as_ref() else {
                    bail!("Missing docker environment")
                };

                env.docker
                    .remove_container(
                        &output.id,
                        Some(
                            bollard::query_parameters::RemoveContainerOptionsBuilder::default()
                                .force(true)
                                .build(),
                        ),
                    )
                    .await?;
                env.untrack_container(&output.id).await;
            }
        }

        Ok(())
    }

    async fn start(
        &mut self,
        new_config: Option<Self::Config>,
        _extra_args: Option<Vec<String>>,
    ) -> Result<()> {
        if let Some(new_config) = new_config {
            self.config = new_config;
            self.client = self.config.local_url();
        }

        self.spawn_output = <Self as NodeT>::spawn(&self.config, &self.docker).await?;
        watch_log_for_panics(
            self.config.log_path(),
            self.config.label(),
            self.failure_tx.clone(),
        );

        self.wait_for_ready(None).await
    }
}

async fn setup_tx_sender_database(
    config: &TxSenderConfig,
    docker: &DockerEnv,
    postgres_container_id: &str,
) -> Result<()> {
    let drop_sql = format!("DROP DATABASE IF EXISTS {}", sql_ident(&config.db_name));
    let create_sql = format!(
        "CREATE DATABASE {} OWNER {}",
        sql_ident(&config.db_name),
        sql_ident(&config.db_user)
    );

    let timeout = Duration::from_secs(90);
    let start = Instant::now();
    let mut last_err: Option<anyhow::Error> = None;

    while start.elapsed() < timeout {
        match run_psql(
            docker,
            postgres_container_id,
            config,
            &[&drop_sql, &create_sql],
        )
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

async fn run_psql(
    docker: &DockerEnv,
    container_id: &str,
    config: &TxSenderConfig,
    statements: &[&str],
) -> Result<()> {
    for sql in statements {
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
            (*sql).to_string(),
        ];
        docker.exec_in_container(container_id, env, cmd).await?;
    }
    Ok(())
}

fn sql_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

async fn wait_for_tx_sender_ready(endpoint: &str, timeout: Option<Duration>) -> Result<()> {
    let timeout = timeout.unwrap_or(Duration::from_secs(90));
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
