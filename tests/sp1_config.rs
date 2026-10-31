//! The framework must write the migrated tables for both prover node kinds.
use citrea_e2e::config::{
    config_to_file, BatchProverConfig, LightClientProverConfig, ProverGuestRunConfig, SP1HostConfig,
};

#[test]
fn prover_configs_serialize_sp1_modes_and_backup_dir() {
    for (mode, name) in [
        (ProverGuestRunConfig::Skip, "skip"),
        (ProverGuestRunConfig::Execute, "execute"),
        (ProverGuestRunConfig::Prove, "prove"),
        (ProverGuestRunConfig::ProveWithFakes, "prove-with-fakes"),
    ] {
        let host = SP1HostConfig {
            tx_backup_dir: Some("/tmp/proof-inputs".into()),
        };
        let batch = BatchProverConfig {
            proving_mode: mode,
            sp1_host: host.clone(),
            ..Default::default()
        };
        let lcp = LightClientProverConfig {
            proving_mode: mode,
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
            assert_eq!(table["proving_mode"].as_str(), Some(name));
            let host = table["sp1_host"].as_table().unwrap();
            assert_eq!(host.len(), 1);
            assert_eq!(host["tx_backup_dir"].as_str(), Some("/tmp/proof-inputs"));
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
fn prover_defaults_execute_with_no_host_options() {
    let batch = BatchProverConfig::default();
    let lcp = LightClientProverConfig::default();
    for (mode, host) in [
        (batch.proving_mode, &batch.sp1_host),
        (lcp.proving_mode, &lcp.sp1_host),
    ] {
        assert_eq!(mode, ProverGuestRunConfig::Execute);
        assert_eq!(*host, SP1HostConfig::default());
    }
    for config in [
        toml::to_string(&batch).unwrap(),
        toml::to_string(&lcp).unwrap(),
    ] {
        let table: toml::Table = toml::from_str(&config).unwrap();
        assert!(table["sp1_host"].as_table().unwrap().is_empty());
    }
    assert_eq!(
        toml::from_str::<BatchProverConfig>("proving_mode = 'execute'\nproof_sampling_number = 0")
            .unwrap(),
        batch
    );
    assert_eq!(
        toml::from_str::<LightClientProverConfig>(
            "proving_mode = 'execute'\nproof_sampling_number = 0\ninitial_da_height = 1"
        )
        .unwrap(),
        lcp
    );
}
