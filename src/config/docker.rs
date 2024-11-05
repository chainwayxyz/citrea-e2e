use std::{fmt::Debug, path::PathBuf};

use serde::Serialize;
use tracing::debug;

use super::{BitcoinConfig, FullL2NodeConfig, NodeKindMarker};
use crate::{
    log_provider::LogPathProvider,
    node::{get_citrea_args, Config, NodeKind},
    utils::get_genesis_path,
};

const DEFAULT_BITCOIN_DOCKER_IMAGE: &str = "bitcoin/bitcoin:28.0";
const DEFAULT_CITREA_DOCKER_IMAGE: &str =
    "chainwayxyz/citrea-test:57f9b46a366413f2e5b04a2bca07abcc21ad0dde";

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
    T: Clone + Serialize + Debug,
    FullL2NodeConfig<T>: NodeKindMarker,
{
    fn from(config: FullL2NodeConfig<T>) -> Self {
        let kind = FullL2NodeConfig::<T>::kind();

        debug!("Converting config {config:?} for {kind} to docker config");

        let args = get_citrea_args(&config);

        Self {
            ports: vec![config.rollup.rpc.bind_port],
            image: config.docker_image.clone().unwrap_or_else(|| {
                let base_img = DEFAULT_CITREA_DOCKER_IMAGE;
                match std::env::var("CI_TEST_MODE") {
                    Ok(v) if v == "1" || v == "true" => format!("{base_img}-ci"),
                    _ => base_img.to_string(),
                }
            }),
            cmd: args,
            log_path: config.dir.join("stdout.log"),
            volume: VolumeConfig {
                name: format!("{kind}"),
                target: format!("/{kind}/data"),
            },
            host_dir: Some(vec![
                config.dir().to_owned().display().to_string(),
                get_genesis_path(&config),
            ]),
            kind,
        }
    }
}
