use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use futures::TryStreamExt;
use tokio::process::Command;

use super::config::BridgeBackendConfig;
use super::docker::DockerEnv;
use super::framework::TestContext;
use super::node::{LogProvider, Node, Restart, SpawnOutput};
use super::Result;
use crate::node::NodeKind;
use crate::test_client::TestClient;

pub struct BridgeBackendNode {
    spawn_output: SpawnOutput,
    pub config: BridgeBackendConfig,
    docker_env: Arc<Option<DockerEnv>>,
}

impl BridgeBackendNode {
    pub async fn new(config: &BridgeBackendConfig, docker: Arc<Option<DockerEnv>>) -> Result<Self> {
        let spawn_output = Self::spawn(config, &docker).await?;

        Ok(Self {
            spawn_output,
            config: config.clone(),
            docker_env: docker,
        })
    }

    // Switch this over to Node signature once we add support for docker to citrea nodes
    async fn spawn(
        config: &BridgeBackendConfig,
        docker: &Arc<Option<DockerEnv>>,
    ) -> Result<SpawnOutput> {
        match docker.as_ref() {
            Some(docker) => docker.spawn(config.into()).await,
            None => <Self as Node>::spawn(config),
        }
    }
}

impl Node for BridgeBackendNode {
    type Config = BridgeBackendConfig;
    type Client = TestClient;

    fn spawn(config: &Self::Config) -> Result<SpawnOutput> {
        let env = config.get_env();
        println!("Running bridge backend with environment variables: {env:?}");

        Command::new("npm run server:dev")
            .kill_on_drop(true)
            .env_clear()
            .envs(env.clone())
            .spawn()
            .context("Failed to spawn bridge backend server process")
            .map(SpawnOutput::Child)?;

        Command::new("npm run worker:dev")
            .kill_on_drop(true)
            .env_clear()
            .envs(env)
            .spawn()
            .context("Failed to spawn bridge backend worker process")
            .map(SpawnOutput::Child)
    }

    fn spawn_output(&mut self) -> &mut SpawnOutput {
        &mut self.spawn_output
    }

    async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<()> {
        println!("Waiting for ready");
        let start = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(30));
        while start.elapsed() < timeout {
            if true
            // TODO: Do this check.
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        anyhow::bail!("Node failed to become ready within the specified timeout")
    }

    fn client(&self) -> &Self::Client {
        &self.client()
    }

    fn config_mut(&mut self) -> &mut Self::Config {
        &mut self.config
    }

    async fn stop(&mut self) -> Result<()> {
        match self.spawn_output() {
            SpawnOutput::Child(process) => {
                process
                    .kill()
                    .await
                    .context("Failed to kill child process")?;
                Ok(())
            }
            SpawnOutput::Container(crate::node::ContainerSpawnOutput { id, .. }) => {
                std::println!("Stopping container {id}");
                let docker = bollard::Docker::connect_with_local_defaults()
                    .context("Failed to connect to Docker")?;
                docker
                    .stop_container(id, Some(bollard::container::StopContainerOptions { t: 10 }))
                    .await
                    .context("Failed to stop Docker container")?;
                Ok(())
            }
        }
    }
}

impl Restart for BridgeBackendNode {
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
                println!("Docker container {} succesfully removed", output.id);
                Ok(())
            }
        }
    }

    async fn start(&mut self, config: Option<Self::Config>) -> Result<()> {
        if let Some(config) = config {
            self.config = config
        }
        self.spawn_output = Self::spawn(&self.config, &self.docker_env).await?;

        self.wait_for_ready(None).await?;

        // Reload wallets after restart
        self.load_wallets().await;

        Ok(())
    }
}

impl LogProvider for BridgeBackendNode {
    fn kind(&self) -> NodeKind {
        NodeKind::BridgeBackend
    }

    fn log_path(&self) -> PathBuf {
        self.config.data_dir.join("regtest").join("debug.log")
    }
}

pub struct BitcoinNodeCluster {
    inner: Vec<BridgeBackendNode>,
}

impl BitcoinNodeCluster {
    pub async fn new(ctx: &TestContext) -> Result<Self> {
        let n_nodes = ctx.config.test_case.n_nodes;
        let mut cluster = Self {
            inner: Vec::with_capacity(n_nodes),
        };
        for config in ctx.config.bitcoin.iter() {
            let node = BridgeBackendNode::new(config, Arc::clone(&ctx.docker)).await?;
            cluster.inner.push(node)
        }

        Ok(cluster)
    }

    pub async fn stop_all(&mut self) -> Result<()> {
        for node in &mut self.inner {
            // RpcApi::stop(node).await?;
            node.stop().await?;
        }
        Ok(())
    }

    pub async fn wait_for_sync(&self, timeout: Duration) -> Result<()> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            // let mut heights = HashSet::new();
            // for node in &self.inner {
            //     let height = node.get_block_count().await?;
            //     heights.insert(height);
            // }

            // if heights.len() == 1 {
            return Ok(());
            // }

            // sleep(Duration::from_secs(1)).await;
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
                        _ => "127.0.0.1".to_string(),
                    };

                    let add_node_arg = format!("{}:{}", ip, to_node.config.p2p_port);
                    from_node.add_node(&add_node_arg).await?;
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, index: usize) -> Option<&BridgeBackendNode> {
        self.inner.get(index)
    }

    #[allow(unused)]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut BridgeBackendNode> {
        self.inner.get_mut(index)
    }
}
