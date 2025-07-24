use super::{
    bitcoin::BitcoinConfig, test_case::TestCaseConfig, FullBatchProverConfig, FullFullNodeConfig,
    FullLightClientProverConfig, FullSequencerConfig,
};
#[cfg(feature = "clementine")]
use crate::config::clementine::ClementineClusterConfig;
#[cfg(feature = "clementine")]
use crate::config::PostgresConfig;

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
    #[cfg(feature = "clementine")]
    pub postgres: PostgresConfig,
}
