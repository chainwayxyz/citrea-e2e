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
    external: bool,
    docker: Arc<Option<DockerEnv>>,
}

impl Postgres {
    pub async fn new(config: &PostgresConfig, docker: Arc<Option<DockerEnv>>) -> Result<Self> {
        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;

        let instance = Self {
            spawn_output,
            config: config.clone(),
            client: (),
            external: false,
            docker: Arc::clone(&docker),
        };

        instance
            .wait_for_ready(Some(Duration::from_secs(60)))
            .await?;

        debug!("Postgres is ready");

        Ok(instance)
    }

    /// Construct a `Postgres` handle that represents an already-running
    /// external shared instance. No container is spawned.
    pub fn external(config: &PostgresConfig) -> Self {
        Self {
            spawn_output: SpawnOutput::Container(ContainerSpawnOutput {
                id: String::new(),
                ip: "127.0.0.1".to_string(),
            }),
            config: config.clone(),
            client: (),
            external: true,
            docker: Arc::new(None),
        }
    }

    pub fn is_external(&self) -> bool {
        self.external
    }

    fn readiness_check_cmd(&self) -> Vec<String> {
        vec![
            "pg_isready".to_string(),
            "-h".to_string(),
            "127.0.0.1".to_string(),
            "-p".to_string(),
            self.config.port.to_string(),
            "-U".to_string(),
            self.config.user.clone(),
            "-d".to_string(),
            "postgres".to_string(),
        ]
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

    async fn stop(&mut self) -> Result<()> {
        if self.external {
            return Ok(());
        }
        // Replicate default impl for container case.
        use crate::traits::SpawnOutput as SO;
        match self.spawn_output() {
            SO::Container(ContainerSpawnOutput { id, .. }) => {
                let docker = bollard::Docker::connect_with_defaults()
                    .context("Failed to connect to Docker")?;
                docker
                    .stop_container(
                        id,
                        Some(bollard::query_parameters::StopContainerOptions {
                            t: Some(10),
                            ..Default::default()
                        }),
                    )
                    .await
                    .context("Failed to stop Postgres container")?;
                Ok(())
            }
            SO::Child(_) => Ok(()),
        }
    }

    async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<()> {
        debug!(
            "Waiting for Postgres to be ready on port {}",
            self.config.port
        );

        if self.external {
            return Ok(());
        }

        let timeout = timeout.unwrap_or(Duration::from_secs(60));
        let start = std::time::Instant::now();

        wait_for_tcp_bound("127.0.0.1", self.config.port, Some(timeout))
            .await
            .context("Postgres port failed to bind")?;

        // Verify the server actually accepts queries by running pg_isready
        // inside the postgres container. Only runs when a docker env is present.
        let Some(docker) = self.docker.as_ref() else {
            return Ok(());
        };

        let mut last_err: Option<anyhow::Error> = None;
        while start.elapsed() < timeout {
            let res = docker
                .exec_in_named_container("postgres", vec![], self.readiness_check_cmd())
                .await;
            match res {
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

#[cfg(test)]
mod tests {
    use super::Postgres;
    use crate::config::PostgresConfig;

    #[test]
    fn readiness_check_uses_configured_port() {
        let postgres = Postgres::external(&PostgresConfig {
            port: 15432,
            ..Default::default()
        });

        assert_eq!(
            postgres.readiness_check_cmd(),
            vec![
                "pg_isready",
                "-h",
                "127.0.0.1",
                "-p",
                "15432",
                "-U",
                "clementine",
                "-d",
                "postgres",
            ]
        );
    }
}
