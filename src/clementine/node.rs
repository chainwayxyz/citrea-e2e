//! # Clementine Node
use std::{fs::File, process::Stdio, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use tokio::process::Command;
use tracing::info;

use crate::{
    client::Client,
    config::config_to_file,
    log_provider::LogPathProvider,
    node::Config,
    traits::{NodeT, Restart, SpawnOutput},
    utils::get_clementine_path,
    Result,
};

pub struct ClementineNode<C: Config + LogPathProvider> {
    spawn_output: SpawnOutput,
    config: C,
    pub client: Client,
}

impl<C: Config + LogPathProvider> ClementineNode<C> {
    pub async fn new(config: &C) -> Result<Self> {
        let spawn_output = Self::spawn(config)?;

        let client = Client::new(config.rpc_bind_host(), config.rpc_bind_port())?;
        Ok(Self {
            spawn_output,
            config: config.clone(),
            client,
        })
    }

    fn spawn(config: &C) -> Result<SpawnOutput> {
        let clementine = get_clementine_path()?;
        let dir = config.dir();

        let kind = C::node_kind();

        let stdout_path = config.log_path();
        let stdout_file = File::create(&stdout_path).context("Failed to create stdout file")?;
        info!(
            "{} stdout logs available at : {}",
            kind,
            stdout_path.display()
        );

        let stderr_path = config.stderr_path();
        let stderr_file = File::create(stderr_path).context("Failed to create stderr file")?;

        let server_arg = match kind {
            crate::node::NodeKind::Verifier => "--verifier-server",
            _ => panic!("Wrong kind {}", kind),
        };

        let config_path = dir.join(format!("{kind}_config.toml"));
        config_to_file(&config.clementine_config(), &config_path)?;

        Command::new(clementine)
            .arg(server_arg)
            .envs(config.env())
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .kill_on_drop(true)
            .spawn()
            .context(format!("Failed to spawn {kind} process"))
            .map(SpawnOutput::Child)
    }
}

#[async_trait]
impl<C> NodeT for ClementineNode<C>
where
    C: Config + LogPathProvider + Send + Sync,
{
    type Config = C;
    type Client = Client;

    fn spawn(config: &Self::Config) -> Result<SpawnOutput> {
        Self::spawn(config)
    }

    fn spawn_output(&mut self) -> &mut SpawnOutput {
        &mut self.spawn_output
    }

    async fn wait_for_ready(&self, _timeout: Option<Duration>) -> Result<()> {
        // let start = Instant::now();
        // let timeout = timeout.unwrap_or(Duration::from_secs(30));
        // while start.elapsed() < timeout {
        // if self
        //     .client
        //     .ledger_get_head_soft_confirmation()
        //     .await
        //     .is_ok()
        // {
        return Ok(());
        // }
        // sleep(Duration::from_millis(500)).await;
        // }
        // anyhow::bail!(
        //     "{} failed to become ready within the specified timeout",
        //     C::node_kind()
        // )
    }

    fn client(&self) -> &Self::Client {
        &self.client
    }

    fn env(&self) -> Vec<(&'static str, &'static str)> {
        self.config.env()
    }

    fn config_mut(&mut self) -> &mut Self::Config {
        &mut self.config
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

#[async_trait]
impl<C> Restart for ClementineNode<C>
where
    C: Config + LogPathProvider + Send + Sync,
{
    async fn wait_until_stopped(&mut self) -> Result<()> {
        self.stop().await?;
        match &mut self.spawn_output {
            SpawnOutput::Child(pid) => pid.wait().await?,
            SpawnOutput::Container(_) => unimplemented!("L2 nodes don't run in docker yet"),
        };
        Ok(())
    }

    async fn start(&mut self, new_config: Option<Self::Config>) -> Result<()> {
        let config = self.config_mut();
        if let Some(new_config) = new_config {
            *config = new_config;
        }
        *self.spawn_output() = Self::spawn(config)?;
        self.wait_for_ready(None).await
    }
}
