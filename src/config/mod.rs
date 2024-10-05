mod bitcoin;
mod docker;
mod rollup;
mod test;
mod test_case;
mod utils;

use std::path::PathBuf;

pub use bitcoin::BitcoinConfig;
pub use bitcoin_da::service::BitcoinServiceConfig;
pub use citrea_sequencer::SequencerConfig;
pub use docker::DockerConfig;
pub use rollup::{default_rollup_config, RollupConfig};
use serde::Serialize;
pub use sov_stf_runner::{
    FullNodeConfig, ProverConfig, RollupPublicKeys, RpcConfig, RunnerConfig, StorageConfig,
};
pub use test::TestConfig;
pub use test_case::{TestCaseConfig, TestCaseEnv};
pub use utils::config_to_file;

use crate::node::{Config, NodeKind};

#[derive(Clone, Debug)]
pub struct FullL2NodeConfig<T> {
    pub node: T,
    pub rollup: RollupConfig,
    pub docker_image: Option<String>,
    pub dir: PathBuf,
    pub env: Vec<(&'static str, &'static str)>,
}

pub type FullSequencerConfig = FullL2NodeConfig<SequencerConfig>;
pub type FullProverConfig = FullL2NodeConfig<ProverConfig>;
pub type FullFullNodeConfig = FullL2NodeConfig<()>;

pub trait NodeKindMarker {
    const KIND: NodeKind;
}

impl NodeKindMarker for FullSequencerConfig {
    const KIND: NodeKind = NodeKind::Sequencer;
}

impl NodeKindMarker for FullProverConfig {
    const KIND: NodeKind = NodeKind::Prover;
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

    fn rpc_bind_host(&self) -> &str {
        &self.rollup.rpc.bind_host
    }

    fn rpc_bind_port(&self) -> u16 {
        self.rollup.rpc.bind_port
    }

    fn env(&self) -> Vec<(&'static str, &'static str)> {
        self.env.clone()
    }

    fn node_config(&self) -> Option<&Self::NodeConfig> {
        if std::mem::size_of::<T>() == 0 {
            None
        } else {
            Some(&self.node)
        }
    }

    fn node_kind() -> NodeKind {
        <Self as NodeKindMarker>::KIND
    }

    fn rollup_config(&self) -> &RollupConfig {
        &self.rollup
    }
}
