use serde::{Deserialize, Serialize};

use crate::PRE_TANGERINE_BRIDGE_INITIALIZE_PARAMS;

/// Rollup Configuration
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SequencerConfig {
    /// Private key of the sequencer
    pub private_key: String,
    /// Min. l2 blocks for sequencer to commit
    pub max_l2_blocks_per_commitment: u64,
    /// Whether or not the sequencer is running in test mode
    pub test_mode: bool,
    /// Limit for the number of deposit transactions to be included in the block
    pub deposit_mempool_fetch_limit: usize,
    /// Sequencer specific mempool config
    pub mempool_conf: SequencerMempoolConfig,
    /// DA layer update loop interval in ms
    pub da_update_interval_ms: u64,
    /// Block production interval in ms
    pub block_production_interval_ms: u64,
    /// Bridge system contract initialize function parameters
    pub bridge_initialize_params: String,
    /// L1 fee rate multiplier
    pub l1_fee_rate_multiplier: f64,
    /// Maximum L1 fee rate (sat/vbyte)
    pub max_l1_fee_rate_sat_vb: u64,
    /// Configuration for the listen mode sequencer
    pub listen_mode_config: Option<ListenModeConfig>,
}

impl Default for SequencerConfig {
    fn default() -> Self {
        SequencerConfig {
            private_key: "1212121212121212121212121212121212121212121212121212121212121212"
                .to_string(),
            max_l2_blocks_per_commitment: 4,
            test_mode: true,
            deposit_mempool_fetch_limit: 10,
            block_production_interval_ms: 100,
            da_update_interval_ms: 100,
            mempool_conf: SequencerMempoolConfig::default(),
            bridge_initialize_params: hex::encode(PRE_TANGERINE_BRIDGE_INITIALIZE_PARAMS),
            l1_fee_rate_multiplier: 1.0,
            max_l1_fee_rate_sat_vb: 10,
            listen_mode_config: None,
        }
    }
}

/// Mempool Config for the sequencer
/// Read: https://github.com/ledgerwatch/erigon/wiki/Transaction-Pool-Design
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SequencerMempoolConfig {
    /// Max number of transactions in the pending sub-pool
    pub pending_tx_limit: u64,
    /// Max megabytes of transactions in the pending sub-pool
    pub pending_tx_size: u64,
    /// Max number of transactions in the queued sub-pool
    pub queue_tx_limit: u64,
    /// Max megabytes of transactions in the queued sub-pool
    pub queue_tx_size: u64,
    /// Max number of transactions in the base-fee sub-pool
    pub base_fee_tx_limit: u64,
    /// Max megabytes of transactions in the base-fee sub-pool
    pub base_fee_tx_size: u64,
    /// Max number of executable transaction slots guaranteed per account
    pub max_account_slots: u64,
    /// Maximum reorg depth for mempool updates (default: 64 blocks = 2 epochs)
    pub max_update_depth: Option<u64>,
    /// Maximum accounts to reload from state at once (default: 100)
    pub max_reload_accounts: Option<usize>,
    /// Maximum lifetime for non-executable transactions in seconds (default: 10800 = 3 hours)
    pub max_tx_lifetime_secs: Option<u64>,
}

impl Default for SequencerMempoolConfig {
    fn default() -> Self {
        Self {
            pending_tx_limit: 100_000,
            pending_tx_size: 200,
            queue_tx_limit: 100_000,
            queue_tx_size: 200,
            base_fee_tx_limit: 100_000,
            base_fee_tx_size: 200,
            max_account_slots: 16,
            max_update_depth: None,
            max_reload_accounts: None,
            max_tx_lifetime_secs: None,
        }
    }
}

/// Configuration for the listen mode sequencer
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ListenModeConfig {
    /// The sequencer client URL to connect to
    pub sequencer_client_url: String,
    /// The number of blocks to sync from the sequencer
    pub sync_blocks_count: u64,
}

impl Default for ListenModeConfig {
    fn default() -> Self {
        ListenModeConfig {
            sequencer_client_url: "http://localhost:8545".to_string(),
            sync_blocks_count: 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;
    use crate::citrea_config::from_toml_path;

    fn create_config_from(content: &str) -> NamedTempFile {
        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(content.as_bytes()).unwrap();
        config_file
    }

    #[test]
    fn test_correct_config_sequencer() {
        let config = r#"
            private_key = "1212121212121212121212121212121212121212121212121212121212121212"
            max_l2_blocks_per_commitment = 123
            test_mode = false
            deposit_mempool_fetch_limit = 10
            da_update_interval_ms = 1000
            block_production_interval_ms = 1000
            bridge_initialize_params = "000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000008ac7230489e80000000000000000000000000000000000000000000000000000000000000000002d4a209fb3a961d8b1f4ec1caa220c6a50b815febc0b689ddf0b9ddfbf99cb74479e41ac0063066369747265611400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a08000000003b9aca006800000000000000000000000000000000000000000000"
            l1_fee_rate_multiplier = 0.75
            max_l1_fee_rate_sat_vb = 10

            [mempool_conf]
            pending_tx_limit = 100000
            pending_tx_size = 200
            queue_tx_limit = 100000
            queue_tx_size = 200
            base_fee_tx_limit = 100000
            base_fee_tx_size = 200
            max_account_slots = 16
        "#;

        let config_file = create_config_from(config);

        let config: SequencerConfig = from_toml_path(config_file.path()).unwrap();

        let expected = SequencerConfig {
            private_key: "1212121212121212121212121212121212121212121212121212121212121212"
                .to_string(),
            max_l2_blocks_per_commitment: 123,
            test_mode: false,
            deposit_mempool_fetch_limit: 10,
            mempool_conf: SequencerMempoolConfig {
                pending_tx_limit: 100000,
                pending_tx_size: 200,
                queue_tx_limit: 100000,
                queue_tx_size: 200,
                base_fee_tx_limit: 100000,
                base_fee_tx_size: 200,
                max_account_slots: 16,
                max_update_depth: None,
                max_reload_accounts: None,
                max_tx_lifetime_secs: None,
            },
            da_update_interval_ms: 1000,
            block_production_interval_ms: 1000,
            bridge_initialize_params: hex::encode(PRE_TANGERINE_BRIDGE_INITIALIZE_PARAMS),
            l1_fee_rate_multiplier: 0.75,
            max_l1_fee_rate_sat_vb: 10,
            listen_mode_config: None,
        };
        assert_eq!(config, expected);
    }

    #[test]
    fn test_correct_config_listen_mode_sequencer() {
        let config = r#"
            private_key = "1212121212121212121212121212121212121212121212121212121212121212"
            max_l2_blocks_per_commitment = 123
            test_mode = false
            deposit_mempool_fetch_limit = 10
            da_update_interval_ms = 1000
            block_production_interval_ms = 1000
            bridge_initialize_params = "000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000008ac7230489e80000000000000000000000000000000000000000000000000000000000000000002d4a209fb3a961d8b1f4ec1caa220c6a50b815febc0b689ddf0b9ddfbf99cb74479e41ac0063066369747265611400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a08000000003b9aca006800000000000000000000000000000000000000000000"
            l1_fee_rate_multiplier = 1.0
            max_l1_fee_rate_sat_vb = 1

            [mempool_conf]
            pending_tx_limit = 100000
            pending_tx_size = 200
            queue_tx_limit = 100000
            queue_tx_size = 200
            base_fee_tx_limit = 100000
            base_fee_tx_size = 200
            max_account_slots = 16

            [listen_mode_config]
            sequencer_client_url = "http://localhost:8545"
            sync_blocks_count = 10
        "#;

        let config_file = create_config_from(config);

        let config: SequencerConfig = from_toml_path(config_file.path()).unwrap();

        let expected = SequencerConfig {
            private_key: "1212121212121212121212121212121212121212121212121212121212121212"
                .to_string(),
            max_l2_blocks_per_commitment: 123,
            test_mode: false,
            deposit_mempool_fetch_limit: 10,
            mempool_conf: SequencerMempoolConfig {
                pending_tx_limit: 100000,
                pending_tx_size: 200,
                queue_tx_limit: 100000,
                queue_tx_size: 200,
                base_fee_tx_limit: 100000,
                base_fee_tx_size: 200,
                max_account_slots: 16,
                max_update_depth: None,
                max_reload_accounts: None,
                max_tx_lifetime_secs: None,
            },
            da_update_interval_ms: 1000,
            block_production_interval_ms: 1000,
            bridge_initialize_params: hex::encode(PRE_TANGERINE_BRIDGE_INITIALIZE_PARAMS),
            l1_fee_rate_multiplier: 1.0,
            max_l1_fee_rate_sat_vb: 1,
            listen_mode_config: Some(ListenModeConfig {
                sequencer_client_url: "http://localhost:8545".to_string(),
                sync_blocks_count: 10,
            }),
        };
        assert_eq!(config, expected);
    }
}
