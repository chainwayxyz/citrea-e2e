mod bitcoin;
mod docker;
mod rollup;
mod test;
mod test_case;
mod utils;

use std::{
    fmt::{self, Debug},
    path::PathBuf,
};

use anyhow::Context;
pub use bitcoin::BitcoinConfig;
pub use docker::DockerConfig;
pub use rollup::RollupConfig;
use serde::Serialize;
pub use test::TestConfig;
pub use test_case::{TestCaseConfig, TestCaseDockerConfig, TestCaseEnv};
pub use utils::config_to_file;

pub use crate::citrea_config::{
    batch_prover::{BatchProverConfig, ProverGuestRunConfig},
    bitcoin::BitcoinServiceConfig,
    light_client_prover::LightClientProverConfig,
    rollup::{FullNodeConfig, RollupPublicKeys, RpcConfig, RunnerConfig, StorageConfig},
    sequencer::{SequencerConfig, SequencerMempoolConfig},
};
use crate::{
    log_provider::LogPathProvider,
    node::{Config, NodeKind},
    Result,
};

#[derive(Clone, Debug, Default)]
pub enum DaLayer {
    #[default]
    Bitcoin,
    MockDa,
}

impl fmt::Display for DaLayer {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DaLayer::Bitcoin => write!(f, "bitcoin"),
            DaLayer::MockDa => write!(f, "mock"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FullL2NodeConfig<T> {
    pub node: T,
    pub rollup: RollupConfig,
    pub docker_image: Option<String>,
    pub dir: PathBuf,
    pub env: Vec<(&'static str, &'static str)>,
    pub da_layer: Option<DaLayer>,
}

impl<T> FullL2NodeConfig<T>
where
    T: Clone + Serialize + Debug,
    FullL2NodeConfig<T>: NodeKindMarker,
{
    pub fn new(
        node: T,
        rollup: RollupConfig,
        docker_image: Option<String>,
        dir: PathBuf,
        env: Vec<(&'static str, &'static str)>,
    ) -> Result<Self> {
        let conf = Self {
            node,
            rollup,
            docker_image,
            dir,
            env,
            da_layer: None,
        };

        let kind = FullL2NodeConfig::<T>::kind();
        let node_config_args = conf.get_node_config_args().unwrap_or_default();
        if let (Some(config), Some(config_path)) = (conf.node_config(), node_config_args.get(1)) {
            config_to_file(config, &config_path)
                .with_context(|| format!("Error writing {kind} config to file"))?;
        }

        let rollup_config_args = conf.get_rollup_config_args();
        config_to_file(&conf.rollup_config(), &rollup_config_args[1])?;

        Ok(conf)
    }
}

pub type FullSequencerConfig = FullL2NodeConfig<SequencerConfig>;
pub type FullBatchProverConfig = FullL2NodeConfig<BatchProverConfig>;
pub type FullLightClientProverConfig = FullL2NodeConfig<LightClientProverConfig>;
pub type FullFullNodeConfig = FullL2NodeConfig<()>;

pub trait NodeKindMarker {
    const KIND: NodeKind;
}

impl NodeKindMarker for FullSequencerConfig {
    const KIND: NodeKind = NodeKind::Sequencer;
}

impl NodeKindMarker for FullBatchProverConfig {
    const KIND: NodeKind = NodeKind::BatchProver;
}

impl NodeKindMarker for FullLightClientProverConfig {
    const KIND: NodeKind = NodeKind::LightClientProver;
}

impl NodeKindMarker for FullFullNodeConfig {
    const KIND: NodeKind = NodeKind::FullNode;
}

impl<T: Clone + Serialize> Config for FullL2NodeConfig<T>
where
    FullL2NodeConfig<T>: NodeKindMarker,
{
    type NodeConfig = T;

    fn dir(&self) -> &PathBuf {
        &self.dir
    }

    fn set_dir(&mut self, new_dir: PathBuf) {
        self.dir = new_dir
    }

    fn rpc_bind_host(&self) -> &str {
        &self.rollup.rpc.bind_host
    }

    fn rpc_bind_port(&self) -> u16 {
        self.rollup.rpc.bind_port
    }

    fn env(&self) -> Vec<(&'static str, &'static str)> {
        self.env.clone()
    }

    fn node_kind() -> NodeKind {
        <Self as NodeKindMarker>::KIND
    }

    fn node_config(&self) -> Option<&Self::NodeConfig> {
        if std::mem::size_of::<T>() == 0 {
            None
        } else {
            Some(&self.node)
        }
    }
    fn rollup_config(&self) -> &RollupConfig {
        &self.rollup
    }

    // Get node config path argument and path.
    // Not required for `full-node`
    fn get_node_config_args(&self) -> Option<Vec<String>> {
        let dir = self.dir();
        let kind = Self::node_kind();
        self.node_config().map(|_| {
            let config_path = dir.join(format!("{kind}_config.toml"));
            let node_kind_str = kind.to_string();
            vec![
                format!("--{node_kind_str}"),
                config_path.display().to_string(),
            ]
        })
    }

    // Get rollup config path argument and path.
    fn get_rollup_config_args(&self) -> Vec<String> {
        let kind = Self::node_kind();
        vec![
            format!("--rollup-config-path"),
            self.dir()
                .join(format!("{kind}_rollup_config.toml"))
                .display()
                .to_string(),
        ]
    }

    fn da_layer(&self) -> DaLayer {
        self.da_layer.clone().unwrap_or_default()
    }
}

impl<T> LogPathProvider for FullL2NodeConfig<T>
where
    T: Clone,
    FullL2NodeConfig<T>: Config,
{
    fn kind() -> NodeKind {
        Self::node_kind()
    }

    fn log_path(&self) -> PathBuf {
        self.dir().join("stdout.log")
    }

    fn stderr_path(&self) -> PathBuf {
        self.dir().join("stderr.log")
    }
}
