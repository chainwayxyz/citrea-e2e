use super::{
    bitcoin::BitcoinConfig, test_case::TestCaseConfig, FullFullNodeConfig, FullBatchProverConfig,
    FullSequencerConfig,
};

#[derive(Clone)]
pub struct TestConfig {
    pub test_case: TestCaseConfig,
    pub bitcoin: Vec<BitcoinConfig>,
    pub sequencer: FullSequencerConfig,
    pub batch_prover: FullBatchProverConfig,
    pub full_node: FullFullNodeConfig,
}
