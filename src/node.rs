use std::{
    fmt,
    fs::File,
    path::PathBuf,
    process::Stdio,
    time::{Duration, SystemTime},
};

use anyhow::{bail, Context};
use async_trait::async_trait;
use serde::Serialize;
use tokio::{
    process::Command,
    time::{sleep, Instant},
};
use tracing::trace;

use crate::{
    client::Client,
    config::{config_to_file, RollupConfig},
    traits::{LogProvider, NodeT, Restart, SpawnOutput},
    utils::{get_citrea_path, get_genesis_path, get_stderr_path, get_stdout_path},
    Result,
};

#[derive(Debug, Clone)]
pub enum NodeKind {
    Bitcoin,
    BatchProver,
    LightClientProver,
    Sequencer,
    FullNode,
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeKind::Bitcoin => write!(f, "bitcoin"),
            NodeKind::BatchProver => write!(f, "batch-prover"),
            NodeKind::LightClientProver => write!(f, "light-client-prover"),
            NodeKind::Sequencer => write!(f, "sequencer"),
            NodeKind::FullNode => write!(f, "full-node"),
        }
    }
}

pub trait Config: Clone {
    type NodeConfig: Serialize;

    fn dir(&self) -> &PathBuf;
    fn rpc_bind_host(&self) -> &str;
    fn rpc_bind_port(&self) -> u16;
    fn env(&self) -> Vec<(&'static str, &'static str)>;
    fn node_config(&self) -> Option<&Self::NodeConfig>;
    fn node_kind() -> NodeKind;
    fn rollup_config(&self) -> &RollupConfig;
}

pub struct Node<C: Config> {
    spawn_output: SpawnOutput,
    config: C,
    pub client: Client,
}

impl<C: Config> Node<C> {
    pub async fn new(config: &C) -> Result<Self> {
        let spawn_output = Self::spawn(config)?;

        let client = Client::new(config.rpc_bind_host(), config.rpc_bind_port())?;
        Ok(Self {
            spawn_output,
            config: config.clone(),
            client,
        })
    }

    fn get_node_config_args(config: &C) -> Result<Vec<String>> {
        let dir = config.dir();
        let kind = C::node_kind();

        config.node_config().map_or(Ok(Vec::new()), |node_config| {
            let config_path = dir.join(format!("{kind}_config.toml"));
            config_to_file(node_config, &config_path)
                .with_context(|| format!("Error writing {kind} config to file"))?;

            Ok(vec![
                format!("--{kind}-config-path"),
                config_path.display().to_string(),
            ])
        })
    }

    fn spawn(config: &C) -> Result<SpawnOutput> {
        let citrea = get_citrea_path();
        let dir = config.dir();

        let kind = C::node_kind();

        let stdout_file =
            File::create(get_stdout_path(dir)).context("Failed to create stdout file")?;
        let stderr_file =
            File::create(get_stderr_path(dir)).context("Failed to create stderr file")?;

        // Handle full node not having any node config
        let node_config_args = Self::get_node_config_args(config)?;

        let rollup_config_path = dir.join(format!("{kind}_rollup_config.toml"));
        config_to_file(&config.rollup_config(), &rollup_config_path)?;

        Command::new(citrea)
            .arg("--da-layer")
            .arg("bitcoin")
            .arg("--rollup-config-path")
            .arg(rollup_config_path)
            .args(node_config_args)
            .arg("--genesis-paths")
            .arg(get_genesis_path(
                dir.parent().expect("Couldn't get parent dir"),
            ))
            .envs(config.env())
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .kill_on_drop(true)
            .spawn()
            .context(format!("Failed to spawn {kind} process"))
            .map(SpawnOutput::Child)
    }

    pub async fn wait_for_l2_height(&self, num: u64, timeout: Option<Duration>) -> Result<()> {
        let start = SystemTime::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(30)); // Default 30 seconds timeout
        loop {
            trace!("Waiting for soft confirmation {}", num);
            let latest_block = self
                .client
                .ledger_get_head_soft_confirmation_height()
                .await?;

            if latest_block >= num {
                break;
            }

            let now = SystemTime::now();
            if start + timeout <= now {
                bail!("Timeout. Latest L2 block is {:?}", latest_block);
            }

            sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }
}

#[async_trait]
impl<C> NodeT for Node<C>
where
    C: Config + Send + Sync,
{
    type Config = C;
    type Client = Client;

    fn spawn(config: &Self::Config) -> Result<SpawnOutput> {
        Self::spawn(config)
    }

    fn spawn_output(&mut self) -> &mut SpawnOutput {
        &mut self.spawn_output
    }

    async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<()> {
        let start = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(30));
        while start.elapsed() < timeout {
            if self
                .client
                .ledger_get_head_soft_confirmation()
                .await
                .is_ok()
            {
                return Ok(());
            }
            sleep(Duration::from_millis(500)).await;
        }
        anyhow::bail!(
            "{} failed to become ready within the specified timeout",
            C::node_kind()
        )
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

impl<C: Config + Send + Sync> LogProvider for Node<C> {
    fn kind(&self) -> NodeKind {
        C::node_kind()
    }

    fn log_path(&self) -> PathBuf {
        get_stdout_path(self.config.dir())
    }
}

#[async_trait]
impl<C> Restart for Node<C>
where
    C: Config + Send + Sync,
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
