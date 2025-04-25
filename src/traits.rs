use std::{sync::Arc, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use bollard::{container::StopContainerOptions, Docker};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use tokio::process::Child;
use tracing::info;

use super::Result;
use crate::docker::{ContainerSpawnOutput, DockerEnv};

#[derive(Debug)]
pub enum SpawnOutput {
    Child(Child),
    Container(ContainerSpawnOutput),
}

/// The Node trait defines the common interface shared between
/// BitcoinNode, BatchProver, LightClientProver, Sequencer and FullNode
#[async_trait]
pub trait NodeT: Send {
    type Config: Send;
    type Client;

    /// Spawn a new node with specific config and return its child
    async fn spawn(config: &Self::Config, docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput>;
    fn spawn_output(&mut self) -> &mut SpawnOutput;

    fn config_mut(&mut self) -> &mut Self::Config;
    fn config(&self) -> &Self::Config;

    /// Stops the running node
    async fn stop(&mut self) -> Result<()> {
        match self.spawn_output() {
            SpawnOutput::Child(process) => {
                if let Some(pid) = process.id() {
                    info!("Killing process {}", pid);
                    signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
                        .context("Failed to send SIGTERM signal to process")?;
                } else {
                    info!("Process ID not found Killing process");
                    process.kill().await.context("Failed to kill process")?;
                }

                Ok(())
            }
            SpawnOutput::Container(ContainerSpawnOutput { id, .. }) => {
                info!("Stopping container {id}");
                let docker =
                    Docker::connect_with_local_defaults().context("Failed to connect to Docker")?;
                docker
                    .stop_container(id, Some(StopContainerOptions { t: 10 }))
                    .await
                    .context("Failed to stop Docker container")?;
                Ok(())
            }
        }
    }

    /// Wait for the node to be reachable by its client.
    async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<()>;

    fn client(&self) -> &Self::Client;

    fn env(&self) -> Vec<(&'static str, &'static str)> {
        Vec::new()
    }
}

// Two patterns supported :
// - Call wait_until_stopped, runs any extra commands needed for testing purposes, call start again.
// - Call restart if you need to wait for node to be fully shutdown and brough back up with new config.
#[async_trait]
pub trait Restart: NodeT + Send {
    async fn wait_until_stopped(&mut self) -> Result<()>;
    async fn start(
        &mut self,
        new_config: Option<Self::Config>,
        extra_args: Option<Vec<String>>,
    ) -> Result<()>;

    // Default implementation to support waiting for node to be fully shutdown and brough back up with new config.
    async fn restart(
        &mut self,
        new_config: Option<Self::Config>,
        extra_args: Option<Vec<String>>,
    ) -> Result<()> {
        self.wait_until_stopped().await?;
        self.start(new_config, extra_args).await
    }
}
