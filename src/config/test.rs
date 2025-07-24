use super::{
    bitcoin::BitcoinConfig, test_case::TestCaseConfig, FullBatchProverConfig, FullFullNodeConfig,
    FullLightClientProverConfig, FullSequencerConfig,
};
#[cfg(feature = "clementine")]
use crate::config::clementine::ClementineClusterConfig;
use crate::config::PostgresConfig;

#[cfg(not(feature = "clementine"))]
#[derive(Clone, Default)]
pub struct ClementineClusterConfig;

#[cfg(not(feature = "clementine"))]
pub fn default_clementine_cluster_config() -> ClementineClusterConfig {
    ClementineClusterConfig
}

#[derive(Clone)]
pub struct TestConfig {
    pub test_case: TestCaseConfig,
    pub bitcoin: Vec<BitcoinConfig>,
    pub sequencer: FullSequencerConfig,
    pub batch_prover: FullBatchProverConfig,
    pub light_client_prover: FullLightClientProverConfig,
    pub full_node: FullFullNodeConfig,
    #[cfg(feature = "clementine")]
    pub clementine: ClementineClusterConfig,
    pub postgres: PostgresConfig,
}
