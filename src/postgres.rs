use std::{sync::Arc, time::Duration};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tracing::debug;

use crate::{
    config::PostgresConfig,
    docker::{ContainerSpawnOutput, DockerEnv},
    traits::{NodeT, SpawnOutput},
    utils::wait_for_tcp_bound,
};

pub struct Postgres {
    spawn_output: SpawnOutput,
    pub config: PostgresConfig,
    pub client: (), // Simple unit client for now
    docker: Arc<Option<DockerEnv>>,
}

impl Postgres {
    pub async fn new(config: &PostgresConfig, docker: Arc<Option<DockerEnv>>) -> Result<Self> {
        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;

        let instance = Self {
            spawn_output,
            config: config.clone(),
            client: (),
            docker: Arc::clone(&docker),
        };

        instance
            .wait_for_ready(Some(Duration::from_secs(60)))
            .await?;

        debug!("Postgres is ready");

        Ok(instance)
    }

    pub fn container_id(&self) -> Option<&str> {
        match &self.spawn_output {
            SpawnOutput::Container(ContainerSpawnOutput { id, .. }) => Some(id),
            SpawnOutput::Child(_) => None,
        }
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

        let timeout = timeout.unwrap_or(Duration::from_secs(60));
        let start = std::time::Instant::now();

        wait_for_tcp_bound("127.0.0.1", self.config.port, Some(timeout))
            .await
            .context("Postgres port failed to bind")?;

        let Some(docker) = self.docker.as_ref() else {
            return Ok(());
        };
        let Some(container_id) = self.container_id() else {
            return Ok(());
        };

        let cmd = vec![
            "pg_isready".to_string(),
            "-h".to_string(),
            "127.0.0.1".to_string(),
            "-p".to_string(),
            self.config.port.to_string(),
            "-U".to_string(),
            self.config.user.clone(),
            "-d".to_string(),
            "postgres".to_string(),
        ];

        let mut last_err: Option<anyhow::Error> = None;
        while start.elapsed() < timeout {
            match docker
                .exec_in_container(container_id, vec![], cmd.clone())
                .await
            {
                Ok(()) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        bail!(
            "Postgres failed to become query-ready: {}",
            last_err.map(|e| format!("{e:#}")).unwrap_or_default()
        )
    }

    fn client(&self) -> &Self::Client {
        &self.client
    }
}
