use super::{
    bitcoin::BitcoinConfig, test_case::TestCaseConfig, FullBatchProverConfig, FullFullNodeConfig,
    FullLightClientProverConfig, FullSequencerConfig,
};

#[derive(Clone, Debug)]
pub struct TestConfig {
    pub test_case: TestCaseConfig,
    pub bitcoin: Vec<BitcoinConfig>,
    pub sequencer: Vec<FullSequencerConfig>,
    pub batch_prover: FullBatchProverConfig,
    pub light_client_prover: FullLightClientProverConfig,
    pub full_node: FullFullNodeConfig,
}
