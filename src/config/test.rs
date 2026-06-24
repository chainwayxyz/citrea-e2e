use super::{
    bitcoin::BitcoinConfig, test_case::TestCaseConfig, FullBatchProverConfig, FullFullNodeConfig,
    FullLightClientProverConfig, FullSequencerConfig,
};
#[cfg(feature = "clementine")]
use crate::config::clementine::ClementineClusterConfig;
use crate::config::{PostgresConfig, TxSenderConfig};

#[derive(Clone, Debug)]
pub struct TestConfig {
    pub test_case: TestCaseConfig,
    pub bitcoin: Vec<BitcoinConfig>,
    pub sequencer: Vec<FullSequencerConfig>,
    pub batch_prover: FullBatchProverConfig,
    pub light_client_prover: FullLightClientProverConfig,
    pub full_node: FullFullNodeConfig,
    pub postgres: PostgresConfig,
    pub tx_sender: Vec<TxSenderConfig>,
    #[cfg(feature = "clementine")]
    pub clementine: ClementineClusterConfig,
}
