#![allow(deprecated)] // Allowing deprecation for now as bollard v0.19.1 has bogus warning messages that cannot be fixed as of now. TODO remove when possible
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::{anyhow, Context, Result};
use bollard::{
    container::{Config, LogOutput, NetworkingConfig},
    models::{EndpointSettings, Mount, PortBinding},
    network::CreateNetworkOptions,
    query_parameters::{CreateContainerOptions, CreateImageOptions, LogsOptions},
    secret::MountTypeEnum,
    service::HostConfig,
    volume::CreateVolumeOptions,
    Docker,
};
use futures::StreamExt;
use tokio::{fs::File, io::AsyncWriteExt, sync::Mutex, task::JoinHandle};
use tracing::{debug, error, info};

use super::{config::DockerConfig, traits::SpawnOutput, utils::generate_test_id};
use crate::{config::TestCaseDockerConfig, node::NodeKind};

#[derive(Debug)]
pub struct ContainerSpawnOutput {
    pub id: String,
    pub ip: String,
}

#[derive(Debug, Clone)]
pub struct NetworkInfo {
    id: String,
    name: String,
}

pub struct DockerEnv {
    pub docker: Docker,
    pub network_info: NetworkInfo,
    id: String,
    volumes: Mutex<HashSet<String>>,
    container_ids: Mutex<HashSet<String>>,
    test_case_config: TestCaseDockerConfig,
}

impl DockerEnv {
    pub async fn new(test_case_config: TestCaseDockerConfig) -> Result<Self> {
        let docker = Docker::connect_with_defaults().context("Failed to connect to Docker")?;
        let test_id = generate_test_id();
        let network_info = Self::create_network(&docker, &test_id).await?;

        Ok(Self {
            docker,
            network_info,
            id: test_id,
            volumes: Mutex::new(HashSet::new()),
            container_ids: Mutex::new(HashSet::new()),
            test_case_config,
        })
    }

    /// Create a volume per node
    /// Keeps track of volumes for cleanup
    async fn create_volume(&self, config: &DockerConfig) -> Result<()> {
        let volume_name = format!("{}-{}", config.volume.name, self.id);
        self.docker
            .create_volume(CreateVolumeOptions {
                name: volume_name.clone(),
                driver: "local".to_string(),
                driver_opts: HashMap::new(),
                labels: HashMap::new(),
            })
            .await?;

        self.volumes.lock().await.insert(volume_name);

        Ok(())
    }

    /// Create a new test network and return its network, name and id
    async fn create_network(docker: &Docker, test_case_id: &str) -> Result<NetworkInfo> {
        let network_name = format!("test_network_{test_case_id}");
        let options = CreateNetworkOptions {
            name: network_name.clone(),
            check_duplicate: true,
            driver: "bridge".to_string(),
            ..Default::default()
        };

        let id = docker.create_network(options).await?.id;

        Ok(NetworkInfo {
            id,
            name: network_name,
        })
    }

    pub fn get_hostname(&self, kind: &NodeKind) -> String {
        format!("{kind}-{}", self.id)
    }

    pub async fn spawn(&self, config: DockerConfig) -> Result<SpawnOutput> {
        debug!("Spawning docker with config {config:#?}");

        self.create_volume(&config).await?;

        let exposed_ports: HashMap<String, HashMap<(), ()>> = config
            .ports
            .iter()
            .map(|port| (format!("{port}/tcp"), HashMap::new()))
            .collect();

        let port_bindings: HashMap<String, Option<Vec<PortBinding>>> = config
            .ports
            .iter()
            .map(|port| {
                (
                    format!("{port}/tcp"),
                    Some(vec![PortBinding {
                        host_ip: Some("0.0.0.0".to_string()),
                        host_port: Some(port.to_string()),
                    }]),
                )
            })
            .collect();

        let mut network_config = HashMap::new();
        network_config.insert(
            self.network_info.id.clone(),
            EndpointSettings {
                ip_address: Some(self.get_hostname(&config.kind)),
                ..Default::default()
            },
        );

        let volume_name = format!("{}-{}", config.volume.name, self.id);
        let mut mounts = vec![Mount {
            target: Some(config.volume.target.clone()),
            source: Some(volume_name),
            typ: Some(MountTypeEnum::VOLUME),
            ..Default::default()
        }];

        if let Some(host_dir) = &config.host_dir {
            for dir in host_dir {
                mounts.push(Mount {
                    target: Some(dir.clone()),
                    source: Some(dir.clone()),
                    typ: Some(MountTypeEnum::BIND),
                    ..Default::default()
                });
            }
        }

        let mut host_config = HostConfig {
            port_bindings: Some(port_bindings),
            mounts: Some(mounts),
            ..Default::default()
        };

        if let Some(throttle) = &config.throttle {
            debug!("Running container with throttle: {:?}", throttle);

            if let Some(cpu) = &throttle.cpu {
                host_config.nano_cpus = Some((cpu.cpus * 1_000_000_000.0) as i64);
            }

            if let Some(memory) = &throttle.memory {
                host_config.memory = Some(memory.limit as i64);
                host_config.oom_kill_disable = Some(true);
            }
        }

        let container_config = Config {
            hostname: Some(format!("{}-{}", config.kind, self.id)),
            image: Some(config.image),
            cmd: Some(config.cmd),
            exposed_ports: Some(exposed_ports),
            env: Some(vec!["PARALLEL_PROOF_LIMIT=1".to_string()]), // Todo proper env handling
            host_config: Some(host_config),
            networking_config: Some(NetworkingConfig {
                endpoints_config: network_config,
            }),
            tty: Some(true),
            ..Default::default()
        };

        let image = container_config
            .image
            .as_ref()
            .context("Image not specified in config")?;
        self.ensure_image_exists(image).await?;

        let container = self
            .docker
            .create_container(None::<CreateContainerOptions>, container_config)
            .await
            .map_err(|e| anyhow!("Failed to create Docker container {e}"))?;

        self.container_ids.lock().await.insert(container.id.clone());

        self.docker
            .start_container(
                &container.id,
                None::<bollard::query_parameters::StartContainerOptions>,
            )
            .await
            .context("Failed to start Docker container")?;

        let inspect_result = self
            .docker
            .inspect_container(
                &container.id,
                None::<bollard::query_parameters::InspectContainerOptions>,
            )
            .await?;
        let ip_address = inspect_result
            .network_settings
            .and_then(|ns| ns.networks)
            .and_then(|networks| {
                networks
                    .values()
                    .next()
                    .and_then(|network| network.ip_address.clone())
            })
            .context("Failed to get container IP address")?;

        // Extract container logs to host
        // This spawns a background task to continuously stream logs from the container.
        // The task will run until the container is stopped or removed during cleanup.
        Self::extract_container_logs(
            self.docker.clone(),
            container.id.clone(),
            config.log_path,
            &config.kind,
        );

        let spawn_output = SpawnOutput::Container(ContainerSpawnOutput {
            id: container.id,
            ip: ip_address,
        });
        debug!("{}, spawn_output : {spawn_output:?}", config.kind);
        Ok(spawn_output)
    }

    async fn ensure_image_exists(&self, image: &str) -> Result<()> {
        let images = self
            .docker
            .list_images(None::<bollard::query_parameters::ListImagesOptions>)
            .await
            .context("Failed to list Docker images")?;
        if images
            .iter()
            .any(|img| img.repo_tags.contains(&image.to_string()))
        {
            return Ok(());
        }

        info!("Pulling image: {image}...");
        let options = Some(CreateImageOptions {
            from_image: Some(image.to_string()),
            ..Default::default()
        });

        let mut stream = self.docker.create_image(options, None, None);
        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let (Some(status), Some(progress)) = (info.status, info.progress) {
                        info!("\r{status}: {progress}     ");
                    }
                }
                Err(e) => return Err(anyhow::anyhow!("Failed to pull image: {}", e)),
            }
        }
        info!("Image succesfully pulled");

        Ok(())
    }

    pub async fn cleanup(&self) -> Result<()> {
        for id in self.container_ids.lock().await.iter() {
            debug!("Logs for container {}:", id);
            let _ = self.dump_logs_cli(id);
        }

        let containers = self
            .docker
            .list_containers(None::<bollard::query_parameters::ListContainersOptions>)
            .await?;
        for container in containers {
            if let (Some(id), Some(networks)) = (
                container.id,
                container.network_settings.and_then(|ns| ns.networks),
            ) {
                if networks.contains_key(&self.network_info.name) {
                    self.docker
                        .stop_container(
                            &id,
                            None::<bollard::query_parameters::StopContainerOptions>,
                        )
                        .await?;
                    self.docker
                        .remove_container(
                            &id,
                            None::<bollard::query_parameters::RemoveContainerOptions>,
                        )
                        .await?;
                }
            }
        }

        self.docker.remove_network(&self.network_info.name).await?;

        for volume_name in self.volumes.lock().await.iter() {
            self.docker
                .remove_volume(volume_name, None::<bollard::volume::RemoveVolumeOptions>)
                .await?;
        }
        Ok(())
    }

    fn extract_container_logs(
        docker: Docker,
        container_id: String,
        log_path: PathBuf,
        kind: &NodeKind,
    ) -> JoinHandle<Result<()>> {
        info!("{} stdout logs available at : {}", kind, log_path.display());

        tokio::spawn(async move {
            if let Some(parent) = log_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .context("Failed to create log directory")?;
            }
            let mut log_file = File::create(log_path)
                .await
                .context("Failed to create log file")?;
            let mut log_stream = docker.logs(
                &container_id,
                Some(LogsOptions {
                    follow: true,
                    stdout: true,
                    stderr: true,
                    tail: "all".to_string(),
                    ..Default::default()
                }),
            );

            while let Some(Ok(log_output)) = log_stream.next().await {
                let log_line = match log_output {
                    LogOutput::Console { message } | LogOutput::StdOut { message } => message,
                    _ => continue,
                };
                log_file
                    .write_all(&log_line)
                    .await
                    .context("Failed to write log line")?;
            }
            Ok(())
        })
    }

    fn dump_logs_cli(&self, container_id: &str) -> Result<()> {
        let n_lines = std::env::var("TAIL_N_LINES").unwrap_or_else(|_| "100".to_string());

        let output = std::process::Command::new("docker")
            .args(["logs", container_id, "-n", &n_lines])
            .output()?;

        debug!("{}", String::from_utf8_lossy(&output.stdout));

        if !output.stderr.is_empty() {
            error!("{}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    // Should run bitcoin in docker
    pub fn bitcoin(&self) -> bool {
        self.test_case_config.bitcoin
    }

    // Should run citrea in docker
    pub fn citrea(&self) -> bool {
        self.test_case_config.citrea
    }

    // Should run clementine in docker
    #[cfg(feature = "clementine")]
    pub fn clementine(&self) -> bool {
        self.test_case_config.clementine
    }
}
