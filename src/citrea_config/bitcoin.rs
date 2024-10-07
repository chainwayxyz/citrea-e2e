use serde::{Deserialize, Serialize};

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
}
