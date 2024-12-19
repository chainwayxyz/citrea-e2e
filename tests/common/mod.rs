use citrea_e2e::config::{BatchProverConfig, LightClientProverConfig, SequencerConfig};

pub type TestFramework = citrea_e2e::framework::TestFramework<
    SequencerConfig,
    BatchProverConfig,
    LightClientProverConfig,
>;
