//! The framework must write the migrated tables for both prover node kinds.
use citrea_e2e::config::{
    config_to_file, BatchProverConfig, LightClientProverConfig, SP1HostConfig, SP1ProverConfig,
};

#[test]
fn prover_configs_serialize_sp1_backends() {
    for (prover, name) in [
        (SP1ProverConfig::Cpu, "cpu"),
        (SP1ProverConfig::Network, "network"),
        (SP1ProverConfig::Mock, "mock"),
    ] {
        let host = SP1HostConfig {
            prover,
            ..Default::default()
        };
        let batch = BatchProverConfig {
            sp1_host: host.clone(),
            ..Default::default()
        };
        let lcp = LightClientProverConfig {
            sp1_host: host,
            ..Default::default()
        };
        let file = tempfile::NamedTempFile::new().unwrap();
        for config in [
            toml::to_string(&batch).unwrap(),
            toml::to_string(&lcp).unwrap(),
        ] {
            let table: toml::Table = toml::from_str(&config).unwrap();
            assert!(!table.contains_key("risc0_host"));
            assert_eq!(table["sp1_host"]["prover"].as_str(), Some(name));
        }
        config_to_file(&batch, &file.path()).unwrap();
        let decoded: BatchProverConfig =
            toml::from_str(&std::fs::read_to_string(file.path()).unwrap()).unwrap();
        assert_eq!(decoded, batch);
        config_to_file(&lcp, &file.path()).unwrap();
        let decoded: LightClientProverConfig =
            toml::from_str(&std::fs::read_to_string(file.path()).unwrap()).unwrap();
        assert_eq!(decoded, lcp);
    }
}

#[test]
fn prover_defaults_select_sp1_cpu() {
    assert_eq!(
        BatchProverConfig::default().sp1_host.prover,
        SP1ProverConfig::Cpu
    );
    assert_eq!(
        LightClientProverConfig::default().sp1_host.prover,
        SP1ProverConfig::Cpu
    );
}
