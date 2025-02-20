use serde::{Deserialize, Serialize};

use crate::PRE_FORK2_BRIDGE_INITIALIZE_PARAMS;

/// Rollup Configuration
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SequencerConfig {
    /// Private key of the sequencer
    pub private_key: String,
    /// Min. soft confirmaitons for sequencer to commit
    pub min_soft_confirmations_per_commitment: u64,
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
}

impl Default for SequencerConfig {
    fn default() -> Self {
        SequencerConfig {
            private_key: "1212121212121212121212121212121212121212121212121212121212121212"
                .to_string(),
            min_soft_confirmations_per_commitment: 4,
            test_mode: true,
            deposit_mempool_fetch_limit: 10,
            block_production_interval_ms: 100,
            da_update_interval_ms: 100,
            mempool_conf: SequencerMempoolConfig::default(),
            bridge_initialize_params: PRE_FORK2_BRIDGE_INITIALIZE_PARAMS.to_string(),
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
            min_soft_confirmations_per_commitment = 123
            test_mode = false
            deposit_mempool_fetch_limit = 10
            da_update_interval_ms = 1000
            block_production_interval_ms = 1000
            bridge_initialize_params = "000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000008ac7230489e80000000000000000000000000000000000000000000000000000000000000000002d4a209fb3a961d8b1f4ec1caa220c6a50b815febc0b689ddf0b9ddfbf99cb74479e41ac0063066369747265611400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a08000000003b9aca006800000000000000000000000000000000000000000000"
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
            min_soft_confirmations_per_commitment: 123,
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
            },
            da_update_interval_ms: 1000,
            block_production_interval_ms: 1000,
            bridge_initialize_params: PRE_FORK2_BRIDGE_INITIALIZE_PARAMS.to_string(),
        };
        assert_eq!(config, expected);
    }
}
