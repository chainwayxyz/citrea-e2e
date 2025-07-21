use std::{sync::Arc, time::Duration};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tracing::{debug, info};

use crate::{
    config::PostgresConfig,
    docker::DockerEnv,
    traits::{NodeT, SpawnOutput},
    utils::wait_for_tcp_bound,
};

pub struct Postgres {
    spawn_output: SpawnOutput,
    pub config: PostgresConfig,
    pub client: (), // Simple unit client for now
}

impl Postgres {
    pub async fn new(config: &PostgresConfig, docker: Arc<Option<DockerEnv>>) -> Result<Self> {
        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;

        let instance = Self {
            spawn_output,
            config: config.clone(),
            client: (),
        };

        instance
            .wait_for_ready(Some(Duration::from_secs(60)))
            .await?;

        debug!("Postgres is ready");

        Ok(instance)
    }
}

#[async_trait]
impl NodeT for Postgres {
    type Config = PostgresConfig;
    type Client = ();

    async fn spawn(config: &Self::Config, docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        match docker.as_ref() {
            Some(docker) => {
                debug!("Spawning Postgres container");
                docker.spawn(config.into()).await
            }
            None => {
                bail!("Postgres requires Docker to be available - cannot run natively")
            }
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
        debug!(
            "Waiting for Postgres to be ready on port {}",
            self.config.port
        );

        wait_for_tcp_bound("127.0.0.1", self.config.port, timeout)
            .await
            .context("Postgres failed to become ready")
    }

    fn client(&self) -> &Self::Client {
        &self.client
    }
}
