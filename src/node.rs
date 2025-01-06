use std::{
    fmt::{self, Debug},
    fs::File,
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};

use anyhow::{bail, Context};
use async_trait::async_trait;
use bitcoincore_rpc::{Auth, Client as BitcoinClient};
use serde::Serialize;
use tokio::{
    process::Command,
    time::{sleep, Instant},
};
use tracing::{debug, info, trace};

use crate::{
    client::Client,
    config::{BitcoinConfig, DaLayer, DockerConfig, RollupConfig},
    docker::DockerEnv,
    log_provider::LogPathProvider,
    traits::{NodeT, Restart, SpawnOutput},
    utils::{copy_directory, get_citrea_path, get_genesis_path},
    Result,
};

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum NodeKind {
    Bitcoin,
    BatchProver,
    LightClientProver,
    Sequencer,
    FullNode,
}

impl NodeKind {
    pub fn to_u8(&self) -> u8 {
        match self {
            NodeKind::Bitcoin => 1,
            NodeKind::BatchProver => 2,
            NodeKind::LightClientProver => 3,
            NodeKind::Sequencer => 4,
            NodeKind::FullNode => 5,
        }
    }
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
    fn set_dir(&mut self, new_dir: PathBuf);
    fn rpc_bind_host(&self) -> &str;
    fn rpc_bind_port(&self) -> u16;
    fn env(&self) -> Vec<(&'static str, &'static str)>;
    fn node_config(&self) -> Option<&Self::NodeConfig>;
    fn node_kind() -> NodeKind;
    fn rollup_config(&self) -> &RollupConfig;
    fn da_layer(&self) -> DaLayer;

    // Get node config path argument and path.
    // Not required for `full-node`
    fn get_node_config_args(&self) -> Option<Vec<String>>;
    fn get_rollup_config_args(&self) -> Vec<String>;
}

pub struct Node<C: Config + LogPathProvider + Send + Sync> {
    spawn_output: SpawnOutput,
    config: C,
    pub client: Client,
    // Bitcoin client targetting node's wallet endpoint
    pub da: BitcoinClient,
}

impl<C> Node<C>
where
    C: Config + LogPathProvider + Send + Sync + Debug,
    DockerConfig: From<C>,
{
    pub async fn new(
        config: &C,
        da_config: &BitcoinConfig,
        docker: Arc<Option<DockerEnv>>,
    ) -> Result<Self> {
        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;

        let client = Client::new(config.rpc_bind_host(), config.rpc_bind_port())?;

        let da_rpc_url = format!(
            "http://127.0.0.1:{}/wallet/{}",
            da_config.rpc_port,
            C::kind()
        );
        let da_client = BitcoinClient::new(
            &da_rpc_url,
            Auth::UserPass(da_config.rpc_user.clone(), da_config.rpc_password.clone()),
        )
        .await
        .context("Failed to create RPC client")?;

        Ok(Self {
            spawn_output,
            config: config.clone(),
            client,
            da: da_client,
        })
    }

    fn spawn(config: &C) -> Result<SpawnOutput> {
        let citrea = get_citrea_path()?;

        let kind = C::node_kind();

        debug!("Spawning {kind} with config {config:?}");

        let stdout_path = config.log_path();
        let stdout_file = File::create(&stdout_path).context("Failed to create stdout file")?;
        info!(
            "{} stdout logs available at : {}",
            kind,
            stdout_path.display()
        );

        let stderr_path = config.stderr_path();
        let stderr_file = File::create(stderr_path).context("Failed to create stderr file")?;

        Command::new(citrea)
            .args(get_citrea_args(config))
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
    C: Config + LogPathProvider + Send + Sync + Debug,
    DockerConfig: From<C>,
{
    type Config = C;
    type Client = Client;

    async fn spawn(config: &Self::Config, docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        match docker.as_ref() {
            Some(docker) if docker.citrea() => docker.spawn(config.to_owned().into()).await,
            _ => Self::spawn(config),
        }
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
                .ledger_get_head_soft_confirmation_height()
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

#[async_trait]
impl<C> Restart for Node<C>
where
    C: Config + LogPathProvider + Send + Sync + Debug,
    DockerConfig: From<C>,
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

        // Update and copy to new dir in order not to overwrite the previous datadir when re-spawning
        // Keep track of multiple restarts by creating {node_kind}-{INDEX} directories per restart
        static INDEX: AtomicU8 = AtomicU8::new(0);
        INDEX.fetch_add(1, Ordering::SeqCst);

        let old_dir = config.dir();
        let new_dir = old_dir.parent().unwrap().join(format!(
            "{}-{}",
            Self::Config::node_kind(),
            INDEX.load(Ordering::SeqCst)
        ));
        copy_directory(old_dir, &new_dir)?;
        config.set_dir(new_dir);

        *self.spawn_output() = Self::spawn(config)?;
        self.wait_for_ready(None).await
    }
}

pub fn get_citrea_args<C>(config: &C) -> Vec<String>
where
    C: Config,
{
    let node_config_args = config.get_node_config_args().unwrap_or_default();
    let rollup_config_args = config.get_rollup_config_args();

    let network_arg = match std::env::var("CITREA_NETWORK") {
        Ok(network) => vec!["--network".to_string(), network],
        _ => vec!["--dev".to_string()],
    };

    [
        network_arg,
        vec!["--da-layer".to_string(), config.da_layer().to_string()],
        node_config_args,
        rollup_config_args,
        vec!["--genesis-paths".to_string(), get_genesis_path(config)],
    ]
    .concat()
}
