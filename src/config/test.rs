use super::{
    bitcoin::BitcoinConfig, test_case::TestCaseConfig, ConfigBounds, FullFullNodeConfig,
    FullL2NodeConfig,
};

#[derive(Clone)]
pub struct TestConfig<S, BP, LP>
where
    S: ConfigBounds,
    BP: ConfigBounds,
    LP: ConfigBounds,
{
    pub test_case: TestCaseConfig,
    pub bitcoin: Vec<BitcoinConfig>,
    pub sequencer: FullL2NodeConfig<S>,
    pub batch_prover: FullL2NodeConfig<BP>,
    pub light_client_prover: FullL2NodeConfig<LP>,
    pub full_node: FullFullNodeConfig,
}
