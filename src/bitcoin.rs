use std::{
    collections::HashSet,
    fs::File,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{bail, Context};
use async_trait::async_trait;
use bitcoin::Address;
use bitcoincore_rpc::{json::AddressType::Bech32m, Auth, Client, RpcApi};
use futures::TryStreamExt;
use tokio::{process::Command, sync::OnceCell, time::sleep};
use tracing::{debug, info, trace};

use super::{
    config::BitcoinConfig,
    docker::DockerEnv,
    framework::TestContext,
    traits::{NodeT, Restart, SpawnOutput},
    Result,
};
use crate::{log_provider::LogPathProvider, node::NodeKind};

pub const FINALITY_DEPTH: u64 = 8;

pub struct BitcoinNode {
    spawn_output: SpawnOutput,
    pub config: BitcoinConfig,
    client: Client,
    gen_addr: OnceCell<Address>,
    docker_env: Arc<Option<DockerEnv>>,
}

impl BitcoinNode {
    pub async fn new(config: &BitcoinConfig, docker: Arc<Option<DockerEnv>>) -> Result<Self> {
        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;

        let rpc_url = format!(
            "http://127.0.0.1:{}/wallet/{}",
            config.rpc_port,
            NodeKind::Bitcoin
        );
        let client = Client::new(
            &rpc_url,
            Auth::UserPass(config.rpc_user.clone(), config.rpc_password.clone()),
        )
        .await
        .context("Failed to create RPC client")?;

        wait_for_rpc_ready(&client, None).await?;

        Ok(Self {
            spawn_output,
            config: config.clone(),
            client,
            gen_addr: OnceCell::new(),
            docker_env: docker,
        })
    }

    pub async fn wait_mempool_len(
        &self,
        target_len: usize,
        timeout: Option<Duration>,
    ) -> Result<()> {
        let timeout = timeout.unwrap_or(Duration::from_secs(300));
        let start = Instant::now();
        while start.elapsed() < timeout {
            let mempool_len = self.get_raw_mempool().await?.len();
            if mempool_len >= target_len {
                return Ok(());
            }
            sleep(Duration::from_millis(500)).await;
        }
        bail!("Timeout waiting for mempool to reach length {}", target_len)
    }

    pub async fn fund_wallet(&self, name: String, blocks: u64) -> Result<()> {
        let rpc_url = format!("http://127.0.0.1:{}/wallet/{}", self.config.rpc_port, name);
        let client = Client::new(
            &rpc_url,
            Auth::UserPass(
                self.config.rpc_user.clone(),
                self.config.rpc_password.clone(),
            ),
        )
        .await
        .context("Failed to create RPC client")?;

        let gen_addr = client
            .get_new_address(None, Some(Bech32m))
            .await?
            .assume_checked();
        client.generate_to_address(blocks, &gen_addr).await?;
        Ok(())
    }

    pub async fn get_finalized_height(&self) -> Result<u64> {
        Ok(self.get_block_count().await? - FINALITY_DEPTH + 1)
    }

    async fn wait_for_shutdown(&self) -> Result<()> {
        let timeout_duration = Duration::from_secs(30);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout_duration {
            if !self.is_process_running().await? {
                info!("Bitcoin daemon has stopped successfully");
                return Ok(());
            }
            sleep(Duration::from_millis(200)).await;
        }

        bail!("Timeout waiting for Bitcoin daemon to stop")
    }

    async fn is_process_running(&self) -> Result<bool> {
        let data_dir = &self.config.data_dir;
        let output = Command::new("pgrep")
            .args(["-f", &format!("bitcoind.*{}", data_dir.display())])
            .output()
            .await?;

        Ok(output.status.success())
    }

    // Infallible, discard already loaded errors
    async fn load_wallets(&self) {
        let _ = self.load_wallet(&NodeKind::Bitcoin.to_string()).await;
        let _ = self.load_wallet(&NodeKind::Sequencer.to_string()).await;
        let _ = self.load_wallet(&NodeKind::BatchProver.to_string()).await;
        let _ = self
            .load_wallet(&NodeKind::LightClientProver.to_string())
            .await;
    }

    fn spawn(config: &BitcoinConfig) -> Result<SpawnOutput> {
        let args = config.args();
        debug!("Running bitcoind with args : {args:?}");

        info!(
            "Bitcoin debug.log available at : {}",
            config.log_path().display()
        );

        let stderr_path = config.stderr_path();
        let stderr_file = File::create(stderr_path).context("Failed to create stderr file")?;

        Command::new("bitcoind")
            .args(&args)
            .kill_on_drop(true)
            .envs(config.env.clone())
            .stderr(Stdio::from(stderr_file))
            .spawn()
            .context("Failed to spawn bitcoind process")
            .map(SpawnOutput::Child)
    }
}

#[async_trait]
impl RpcApi for BitcoinNode {
    async fn call<T: for<'a> serde::de::Deserialize<'a>>(
        &self,
        cmd: &str,
        args: &[serde_json::Value],
    ) -> bitcoincore_rpc::Result<T> {
        self.client.call(cmd, args).await
    }

    // Override deprecated generate method.
    // Uses or lazy init gen_addr and forward to `generate_to_address`
    async fn generate(
        &self,
        block_num: u64,
        _maxtries: Option<u64>,
    ) -> bitcoincore_rpc::Result<Vec<bitcoin::BlockHash>> {
        let addr = self
            .gen_addr
            .get_or_init(|| async {
                self.client
                    .get_new_address(None, Some(Bech32m))
                    .await
                    .expect("Failed to generate address")
                    .assume_checked()
            })
            .await;

        self.generate_to_address(block_num, addr).await
    }
}

#[async_trait]
impl NodeT for BitcoinNode {
    type Config = BitcoinConfig;
    type Client = Client;

    async fn spawn(config: &Self::Config, docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        match docker.as_ref() {
            Some(docker) if docker.bitcoin() => docker.spawn(config.into()).await,
            _ => Self::spawn(config),
        }
    }

    fn spawn_output(&mut self) -> &mut SpawnOutput {
        &mut self.spawn_output
    }

    async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<()> {
        trace!("Waiting for ready");
        let start = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(30));
        while start.elapsed() < timeout {
            if wait_for_rpc_ready(&self.client, Some(timeout))
                .await
                .is_ok()
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        bail!("Node failed to become ready within the specified timeout")
    }

    fn client(&self) -> &Self::Client {
        &self.client
    }

    fn env(&self) -> Vec<(&'static str, &'static str)> {
        self.config.env.clone()
    }

    fn config_mut(&mut self) -> &mut Self::Config {
        &mut self.config
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

#[async_trait]
impl Restart for BitcoinNode {
    async fn wait_until_stopped(&mut self) -> Result<()> {
        self.client.stop().await?;
        self.stop().await?;

        match &self.spawn_output {
            SpawnOutput::Child(_) => self.wait_for_shutdown().await,
            SpawnOutput::Container(output) => {
                let Some(env) = self.docker_env.as_ref() else {
                    bail!("Missing docker environment")
                };
                env.docker.stop_container(&output.id, None).await?;

                env.docker
                    .wait_container::<String>(&output.id, None)
                    .try_collect::<Vec<_>>()
                    .await?;
                env.docker.remove_container(&output.id, None).await?;
                info!("Docker container {} succesfully removed", output.id);
                Ok(())
            }
        }
    }

    async fn start(&mut self, config: Option<Self::Config>) -> Result<()> {
        if let Some(config) = config {
            self.config = config;
        }
        self.spawn_output = <Self as NodeT>::spawn(&self.config, &self.docker_env).await?;

        self.wait_for_ready(None).await?;

        // Reload wallets after restart
        self.load_wallets().await;

        Ok(())
    }
}

pub struct BitcoinNodeCluster {
    inner: Vec<BitcoinNode>,
}

impl BitcoinNodeCluster {
    pub async fn new(ctx: &TestContext) -> Result<Self> {
        let n_nodes = ctx.config.test_case.n_nodes;
        let mut cluster = Self {
            inner: Vec::with_capacity(n_nodes),
        };
        for config in &ctx.config.bitcoin {
            let node = BitcoinNode::new(config, Arc::clone(&ctx.docker)).await?;
            cluster.inner.push(node);
        }

        Ok(cluster)
    }

    pub async fn stop_all(&mut self) -> Result<()> {
        for node in &mut self.inner {
            RpcApi::stop(node).await?;
            node.stop().await?;
        }
        Ok(())
    }

    pub async fn wait_for_sync(&self, timeout: Duration) -> Result<()> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            let mut heights = HashSet::new();
            for node in &self.inner {
                let height = node.get_block_count().await?;
                heights.insert(height);
            }

            if heights.len() == 1 {
                return Ok(());
            }

            sleep(Duration::from_secs(1)).await;
        }
        bail!("Nodes failed to sync within the specified timeout")
    }

    // Connect all bitcoin nodes between them
    pub async fn connect_nodes(&self) -> Result<()> {
        for (i, from_node) in self.inner.iter().enumerate() {
            for (j, to_node) in self.inner.iter().enumerate() {
                if i != j {
                    let ip = match &to_node.spawn_output {
                        SpawnOutput::Container(container) => container.ip.clone(),
                        SpawnOutput::Child(_) => "127.0.0.1".to_string(),
                    };

                    let add_node_arg = format!("{}:{}", ip, to_node.config.p2p_port);
                    from_node.add_node(&add_node_arg).await?;
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, index: usize) -> Option<&BitcoinNode> {
        self.inner.get(index)
    }

    #[allow(unused)]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut BitcoinNode> {
        self.inner.get_mut(index)
    }
}

async fn wait_for_rpc_ready(client: &Client, timeout: Option<Duration>) -> Result<()> {
    let start = Instant::now();
    let timeout = timeout.unwrap_or(Duration::from_secs(300));
    while start.elapsed() < timeout {
        match client.get_blockchain_info().await {
            Ok(_) => return Ok(()),
            Err(e) => {
                trace!("[wait_for_rpc_ready] error {e}");
                sleep(Duration::from_millis(500)).await;
            }
        }
    }
    bail!("Timeout waiting for RPC to be ready")
}
