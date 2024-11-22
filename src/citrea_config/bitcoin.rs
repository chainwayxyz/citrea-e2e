use serde::{Deserialize, Serialize};

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            check_interval: 1,
            history_limit: 100,
            max_history_size: 1_000_000, // Default to 1mb for test
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MonitoringConfig {
    pub check_interval: u64,
    pub history_limit: usize,
    pub max_history_size: usize,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FeeServiceConfig {
    max_da_bandwidth_bytes: u64,
    window_duration_secs: u64,
    capacity_threshold: f64,
    base_fee_multiplier: f64,
    max_fee_multiplier: f64,
    fee_exponential_factor: f64,
    fee_multipler_scalar: f64,
}

// Default FeeServiceConfig for e2e test.
// Bandwidth duration and size are reduced to 4096/30s for ease of testing
impl Default for FeeServiceConfig {
    fn default() -> Self {
        Self {
            max_da_bandwidth_bytes: 4 * 1024, // 4096 bytes
            window_duration_secs: 30,         // 30secs
            capacity_threshold: 0.5,
            base_fee_multiplier: 1.0,
            max_fee_multiplier: 4.0,
            fee_exponential_factor: 4.0,
            fee_multipler_scalar: 10.0,
        }
    }
}

/// Runtime configuration for the DA service
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BitcoinServiceConfig {
    /// The URL of the Bitcoin node to connect to
    pub node_url: String,
    pub node_username: String,
    pub node_password: String,

    // network of the bitcoin node
    pub network: bitcoin::Network,

    // da private key of the sequencer
    pub da_private_key: Option<String>,

    // absolute path to the directory where the txs will be written to
    pub tx_backup_dir: String,

    pub monitoring: Option<MonitoringConfig>,
    pub fee: Option<FeeServiceConfig>,
}
