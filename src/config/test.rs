use super::{
    bitcoin::BitcoinConfig, test_case::TestCaseConfig, FullBatchProverConfig, FullFullNodeConfig,
    FullLightClientProverConfig, FullSequencerConfig, FullVerifierConfig,
};

#[derive(Clone)]
pub struct TestConfig {
    pub test_case: TestCaseConfig,
    pub bitcoin: Vec<BitcoinConfig>,
    pub sequencer: FullSequencerConfig,
    pub batch_prover: FullBatchProverConfig,
    pub light_client_prover: FullLightClientProverConfig,
    pub verifier: FullVerifierConfig,
    pub full_node: FullFullNodeConfig,
}
