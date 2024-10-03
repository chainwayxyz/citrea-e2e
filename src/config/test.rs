use super::{
    bitcoin::BitcoinConfig, test_case::TestCaseConfig, FullFullNodeConfig, FullProverConfig,
    FullSequencerConfig, BridgeBackendConfig
};

#[derive(Clone)]
pub struct TestConfig {
    pub test_case: TestCaseConfig,
    pub bitcoin: Vec<BitcoinConfig>,
    pub bridge_backend: BridgeBackendConfig,
    pub sequencer: FullSequencerConfig,
    pub prover: FullProverConfig,
    pub full_node: FullFullNodeConfig,
}
