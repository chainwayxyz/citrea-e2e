use std::{fmt::Debug, path::PathBuf};

use serde::Serialize;
use tracing::debug;

use super::{BitcoinConfig, FullL2NodeConfig};
use crate::{
    node::{get_citrea_args, NodeKind},
    utils::get_genesis_path,
};

const DEFAULT_BITCOIN_DOCKER_IMAGE: &str = "bitcoin/bitcoin:28.0";
const DEFAULT_CITREA_DOCKER_IMAGE: &str =
    "chainwayxyz/citrea-test:e2af7f97c62c9c732f6684e6d7d4bad056059480";

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
        }
    }
}
