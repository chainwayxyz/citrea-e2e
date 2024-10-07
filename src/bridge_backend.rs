use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use async_trait::async_trait;
use futures::TryStreamExt;
use tokio::process::Command;
use tokio::time::sleep;

use super::config::BridgeBackendConfig;
use super::docker::DockerEnv;
use super::Result;
use crate::client::Client;
use crate::node::NodeKind;
use crate::traits::{ContainerSpawnOutput, LogProvider, NodeT, Restart, SpawnOutput};

pub struct BridgeBackendNode {
    spawn_output: SpawnOutput,
    pub config: BridgeBackendConfig,
    docker_env: Arc<Option<DockerEnv>>,
    client: Client,
}

impl BridgeBackendNode {
    pub async fn new(config: &BridgeBackendConfig, docker: Arc<Option<DockerEnv>>) -> Result<Self> {
        let spawn_output = Self::spawn(config, &docker).await?;

        Ok(Self {
            spawn_output,
            config: config.clone(),
            docker_env: docker,
            client: Client::new(&config.client.host, config.client.port.try_into().unwrap())?,
        })
    }

    async fn spawn(
        config: &BridgeBackendConfig,
        docker: &Arc<Option<DockerEnv>>,
    ) -> Result<SpawnOutput> {
        match docker.as_ref() {
            Some(docker) => docker.spawn(config.into()).await,
            None => <Self as NodeT>::spawn(config),
        }
    }

    async fn wait_for_shutdown(&self) -> Result<()> {
        let timeout_duration = Duration::from_secs(30);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout_duration {
            if !self.is_process_running().await? {
                println!("Bridge backend has stopped successfully");
                return Ok(());
            }
            sleep(Duration::from_millis(200)).await;
        }

        bail!("Timeout waiting for bridge backend to stop")
    }

    async fn is_process_running(&self) -> Result<bool> {
        // let data_dir = &self.config.data_dir;
        // let output = Command::new("pgrep")
        //     .args(["-f", &format!("bitcoind.*{}", data_dir.display())])
        //     .output()
        //     .await?;

        // Ok(output.status.success())
        todo!()
    }
}

#[async_trait]
impl NodeT for BridgeBackendNode {
    type Config = BridgeBackendConfig;
    type Client = Client;

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
            SpawnOutput::Container(ContainerSpawnOutput { id, .. }) => {
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

    fn client(&self) -> &Self::Client {
        &self.client
    }

    fn env(&self) -> Vec<(&'static str, &'static str)> {
        // self.config.get_env()
        todo!()
    }

    fn config(&self) -> &<Self as NodeT>::Config {
        &self.config
    }
}

#[async_trait]
impl Restart for BridgeBackendNode {
    async fn wait_until_stopped(&mut self) -> Result<()> {
        // self.client.stop().await?;
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

        Ok(())
    }
}

impl LogProvider for BridgeBackendNode {
    fn kind(&self) -> NodeKind {
        NodeKind::BridgeBackend
    }

    fn log_path(&self) -> PathBuf {
        todo!()
    }
}
