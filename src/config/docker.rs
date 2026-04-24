use std::{collections::HashMap, fmt::Debug, path::PathBuf};

use serde::Serialize;
use tracing::debug;

use super::{throttle::ThrottleConfig, BitcoinConfig, FullL2NodeConfig};
use crate::{
    config::{PostgresConfig, TxSenderConfig},
    log_provider::LogPathProvider,
    node::{get_citrea_args, NodeKind},
    utils::get_genesis_path,
};

const DEFAULT_BITCOIN_DOCKER_IMAGE: &str = "bitcoin/bitcoin:30.2";
const DEFAULT_CITREA_DOCKER_IMAGE: &str = "chainwayxyz/citrea-test:latest";
const DEFAULT_TX_SENDER_DOCKER_IMAGE: &str = "chainwayxyz/tx-sender:v0.6.6-alpha.4";

#[derive(Debug)]
pub struct VolumeConfig {
    pub name: String,
    pub target: String,
}

#[derive(Debug)]
pub struct DockerConfig {
    pub name: Option<String>,
    pub ports: Vec<u16>,
    pub image: String,
    pub cmd: Vec<String>,
    pub log_path: PathBuf,
    pub volume: Option<VolumeConfig>,
    pub host_dir: Option<Vec<String>>,
    pub kind: NodeKind,
    pub throttle: Option<ThrottleConfig>,
    pub env: HashMap<String, String>,
    pub extra_hosts: Vec<String>,
}

impl DockerConfig {
    pub(crate) fn new(kind: NodeKind, image: String, cmd: Vec<String>, log_path: PathBuf) -> Self {
        Self {
            name: None,
            ports: Vec::new(),
            image,
            cmd,
            log_path,
            volume: None,
            host_dir: None,
            kind,
            throttle: None,
            env: HashMap::new(),
            extra_hosts: Vec::new(),
        }
    }
}

impl From<&BitcoinConfig> for DockerConfig {
    fn from(config: &BitcoinConfig) -> Self {
        let mut args = config.args();

        // Docker specific args
        args.extend([
            "-rpcallowip=0.0.0.0/0".to_string(),
            "-rpcbind=0.0.0.0".to_string(),
            "-daemonwait=0".to_string(),
        ]);

        let mut docker = Self::new(
            NodeKind::Bitcoin,
            config
                .docker_image
                .clone()
                .unwrap_or_else(|| DEFAULT_BITCOIN_DOCKER_IMAGE.to_string()),
            args,
            config.data_dir.join("regtest").join("debug.log"),
        );
        docker.ports = vec![config.rpc_port, config.p2p_port];
        docker.volume = Some(VolumeConfig {
            name: format!("bitcoin-{}", config.idx),
            target: "/home/bitcoin/.bitcoin".to_string(),
        });
        docker.throttle = None; // Not supported for bitcoin yet. Easy to toggle if it ever makes sense to throttle bitcoind nodes
        docker
    }
}

impl<T> From<FullL2NodeConfig<T>> for DockerConfig
where
    T: Clone + Debug + Serialize + Send + Sync,
{
    fn from(config: FullL2NodeConfig<T>) -> Self {
        let kind = config.kind();

        debug!("Converting config {config:?} for {kind} to docker config");

        let args = get_citrea_args(&config);

        let mut docker = Self::new(
            kind,
            config
                .base
                .docker_image
                .clone()
                .unwrap_or(DEFAULT_CITREA_DOCKER_IMAGE.to_string()),
            args,
            config.dir().join("stdout.log"),
        );
        docker.ports = vec![config.rollup.rpc.bind_port];
        docker.host_dir = Some(vec![
            config.dir().to_owned().display().to_string(),
            get_genesis_path(config.dir()),
            config.rollup.storage.path.display().to_string(),
        ]);
        docker.throttle = config.throttle.clone();
        docker
    }
}

impl From<&PostgresConfig> for DockerConfig {
    fn from(config: &PostgresConfig) -> Self {
        let image_tag = config.image_tag.as_deref().unwrap_or("15");
        let image = format!("postgres:{image_tag}");

        let mut cmd = vec![
            "bash".to_string(),
            "-c".to_string(),
            format!(
                "POSTGRES_PASSWORD={} POSTGRES_USER={} exec docker-entrypoint.sh postgres -p {} {}",
                config.password,
                config.user,
                config.port,
                config.extra_args.join(" ")
            )
            .to_string(),
        ];
        cmd.extend(config.extra_args.clone());

        let mut docker = Self::new(NodeKind::Postgres, image, cmd, config.log_path());
        docker.ports = vec![config.port];
        docker.volume = Some(VolumeConfig {
            name: "postgres".to_string(),
            target: "/var/lib/postgresql/data".to_string(),
        });
        docker
    }
}

impl From<&TxSenderConfig> for DockerConfig {
    fn from(config: &TxSenderConfig) -> Self {
        let mut docker = Self::new(
            NodeKind::TxSender,
            config
                .docker_image
                .clone()
                .unwrap_or_else(|| DEFAULT_TX_SENDER_DOCKER_IMAGE.to_string()),
            vec!["/app/clementine-tx-sender".to_string()],
            config.log_path(),
        );
        docker.name = Some(config.alias());
        docker.ports = vec![config.rpc_port];
        docker.env = config.docker_env();
        // Map host.docker.internal to the host gateway so the tx-sender can reach
        // host-published services (e.g. the shared postgres on docker's default bridge).
        // Docker Desktop resolves host.docker.internal by default, but Linux needs the
        // explicit host-gateway extra host entry.
        docker.extra_hosts = vec!["host.docker.internal:host-gateway".to_string()];
        docker
    }
}
