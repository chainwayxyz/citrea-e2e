use serde::{Deserialize, Serialize};

use crate::bitcoin::FINALITY_DEPTH;

// Test values for MonitoringConfig
impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            check_interval: 1,
            history_limit: 100,
            reorg_depth_threshold: FINALITY_DEPTH,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MonitoringConfig {
    pub check_interval: u64,
    pub history_limit: usize,
    pub reorg_depth_threshold: u64,
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

    pub monitoring: MonitoringConfig,
}
