use std::{collections::HashMap, fmt::Debug, path::PathBuf};

use serde::Serialize;
use tracing::debug;

use super::{throttle::ThrottleConfig, BitcoinConfig, FullL2NodeConfig};
#[cfg(feature = "clementine")]
use crate::config::PostgresConfig;
#[cfg(feature = "clementine")]
use crate::log_provider::LogPathProvider;
use crate::{
    node::{get_citrea_args, NodeKind},
    utils::get_genesis_path,
};

const DEFAULT_BITCOIN_DOCKER_IMAGE: &str = "bitcoin/bitcoin:29.0";
const DEFAULT_CITREA_DOCKER_IMAGE: &str = "chainwayxyz/citrea-test:latest";

#[derive(Debug)]
pub struct VolumeConfig {
    pub name: String,
    pub target: String,
}

#[derive(Debug)]
pub struct DockerConfig {
    pub ports: Vec<u16>,
    pub image: String,
    pub cmd: Vec<String>,
    pub log_path: PathBuf,
    pub volume: VolumeConfig,
    pub host_dir: Option<Vec<String>>,
    pub kind: NodeKind,
    pub throttle: Option<ThrottleConfig>,
    pub env: HashMap<String, String>,
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

        Self {
            ports: vec![config.rpc_port, config.p2p_port],
            image: config
                .docker_image
                .clone()
                .unwrap_or_else(|| DEFAULT_BITCOIN_DOCKER_IMAGE.to_string()),
            cmd: args,
            log_path: config.data_dir.join("regtest").join("debug.log"),
            volume: VolumeConfig {
                name: format!("bitcoin-{}", config.idx),
                target: "/home/bitcoin/.bitcoin".to_string(),
            },
            host_dir: None,
            kind: NodeKind::Bitcoin,
            throttle: None, // Not supported for bitcoin yet. Easy to toggle if it ever makes sense to throttle bitcoind nodes
            env: HashMap::new(),
        }
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

        Self {
            ports: vec![config.rollup.rpc.bind_port],
            image: config
                .base
                .docker_image
                .clone()
                .unwrap_or(DEFAULT_CITREA_DOCKER_IMAGE.to_string()),
            cmd: args,
            log_path: config.dir().join("stdout.log"),
            volume: VolumeConfig {
                name: format!("{kind}"),
                target: format!("/{kind}/data"),
            },
            host_dir: Some(vec![
                config.dir().to_owned().display().to_string(),
                get_genesis_path(config.dir()),
            ]),
            kind,
            throttle: config.throttle.clone(),
            env: HashMap::new(),
        }
    }
}

#[cfg(feature = "clementine")]
impl From<&PostgresConfig> for DockerConfig {
    fn from(config: &PostgresConfig) -> Self {
        let image_tag = config.image_tag.as_deref().unwrap_or("15");
        let image = format!("postgres:{}", image_tag);

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

        Self {
            ports: vec![config.port],
            image,
            cmd,
            log_path: config.log_path(),
            volume: VolumeConfig {
                name: "postgres".to_string(),
                target: "/var/lib/postgresql/data".to_string(),
            },
            host_dir: None,
            kind: NodeKind::Postgres,
            throttle: None,
            env: HashMap::new(),
        }
    }
}
