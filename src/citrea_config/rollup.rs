use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use super::bitcoin::MonitoringConfig;
use crate::config::{BitcoinConfig, BitcoinServiceConfig};

/// Runner configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RunnerConfig {
    /// Sequencer client configuration.
    pub sequencer_client_url: String,
    /// Saves sequencer soft confirmations if set to true
    pub include_tx_body: bool,
    /// Number of blocks to request during sync
    #[serde(default = "default_sync_blocks_count")]
    pub sync_blocks_count: u64,
    /// Configurations for pruning
    pub pruning_config: Option<PruningConfig>,
}

/// RPC configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Default, Serialize)]
pub struct RpcConfig {
    /// RPC host.
    pub bind_host: String,
    /// RPC port.
    pub bind_port: u16,
    /// Maximum number of concurrent requests.
    /// if not set defaults to 100.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Max request body request
    #[serde(default = "default_max_request_body_size")]
    pub max_request_body_size: u32,
    /// Max response body request
    #[serde(default = "default_max_response_body_size")]
    pub max_response_body_size: u32,
    /// Maximum number of batch requests
    #[serde(default = "default_batch_requests_limit")]
    pub batch_requests_limit: u32,
    /// Disable subscription RPCs
    #[serde(default = "default_enable_subscriptions")]
    pub enable_subscriptions: bool,
    /// Maximum number of subscription connections
    #[serde(default = "default_max_subscriptions_per_connection")]
    pub max_subscriptions_per_connection: u32,
}

#[inline]
const fn default_max_connections() -> u32 {
    100
}

#[inline]
const fn default_max_request_body_size() -> u32 {
    10 * 1024 * 1024
}

#[inline]
const fn default_max_response_body_size() -> u32 {
    10 * 1024 * 1024
}

#[inline]
const fn default_batch_requests_limit() -> u32 {
    50
}

#[inline]
const fn default_sync_blocks_count() -> u64 {
    10
}

#[inline]
const fn default_enable_subscriptions() -> bool {
    true
}

#[inline]
const fn default_max_subscriptions_per_connection() -> u32 {
    100
}

/// Simple storage configuration
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct StorageConfig {
    /// Path that can be utilized by concrete rollup implementation
    pub path: PathBuf,
    /// File descriptor limit for `RocksDB`
    pub db_max_open_files: Option<i32>,
}

/// Important public keys for the rollup
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RollupPublicKeys {
    /// Soft confirmation signing public key of the Sequencer
    #[serde(with = "hex::serde")]
    pub sequencer_public_key: Vec<u8>,
    /// DA Signing Public Key of the Sequencer
    /// serialized as hex
    #[serde(with = "hex::serde")]
    pub sequencer_da_pub_key: Vec<u8>,
    /// DA Signing Public Key of the Prover
    /// serialized as hex
    #[serde(with = "hex::serde")]
    pub prover_da_pub_key: Vec<u8>,
}

/// Rollup Configuration
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FullNodeConfig<BitcoinServiceConfig> {
    /// RPC configuration
    pub rpc: RpcConfig,
    /// Currently rollup config runner only supports storage path parameter
    pub storage: StorageConfig,
    /// Runner own configuration.
    pub runner: Option<RunnerConfig>, // optional bc sequencer doesn't need it
    /// Data Availability service configuration.
    pub da: BitcoinServiceConfig,
    /// Important pubkeys
    pub public_keys: RollupPublicKeys,
}

impl Default for FullNodeConfig<BitcoinServiceConfig> {
    fn default() -> Self {
        Self {
            rpc: RpcConfig {
                bind_host: "127.0.0.1".into(),
                bind_port: 0,
                max_connections: 100,
                max_request_body_size: 10 * 1024 * 1024,
                max_response_body_size: 10 * 1024 * 1024,
                batch_requests_limit: 50,
                enable_subscriptions: true,
                max_subscriptions_per_connection: 100,
            },
            storage: StorageConfig {
                path: TempDir::new()
                    .expect("Failed to create temporary directory")
                    .into_path(),
                db_max_open_files: None,
            },
            runner: None,
            da: BitcoinServiceConfig {
                node_url: String::new(),
                node_username: String::from("user"),
                node_password: String::from("password"),
                network: bitcoin::Network::Regtest,
                da_private_key: None,
                tx_backup_dir: TempDir::new()
                    .expect("Failed to create temporary directory")
                    .into_path()
                    .display()
                    .to_string(),
                monitoring: MonitoringConfig::default(),
            },
            public_keys: RollupPublicKeys {
                sequencer_public_key: vec![
                    32, 64, 64, 227, 100, 193, 15, 43, 236, 156, 31, 229, 0, 161, 205, 76, 36, 124,
                    137, 214, 80, 160, 30, 215, 232, 44, 171, 168, 103, 135, 124, 33,
                ],
                // private key [4, 95, 252, 129, 163, 193, 253, 179, 175, 19, 89, 219, 242, 209, 20, 176, 179, 239, 191, 127, 41, 204, 156, 93, 160, 18, 103, 170, 57, 210, 199, 141]
                // Private Key (WIF): KwNDSCvKqZqFWLWN1cUzvMiJQ7ck6ZKqR6XBqVKyftPZtvmbE6YD
                sequencer_da_pub_key: vec![
                    3, 136, 195, 18, 11, 187, 25, 37, 38, 109, 184, 237, 247, 208, 131, 219, 162,
                    70, 35, 174, 234, 47, 239, 247, 60, 51, 174, 242, 247, 112, 186, 222, 30,
                ],
                // private key [117, 186, 249, 100, 208, 116, 89, 70, 0, 54, 110, 91, 17, 26, 29, 168, 248, 107, 46, 254, 45, 34, 218, 81, 200, 216, 33, 38, 160, 252, 172, 114]
                // Private Key (WIF): L1AZdJXzDGGENBBPZGSL7dKJnwn5xSKqzszgK6CDwiBGThYQEVTo
                prover_da_pub_key: vec![
                    2, 138, 232, 157, 214, 46, 7, 210, 235, 33, 105, 239, 71, 169, 105, 233, 239,
                    84, 172, 112, 13, 54, 9, 206, 106, 138, 251, 218, 15, 28, 137, 112, 127,
                ],
            },
        }
    }
}

impl From<BitcoinConfig> for BitcoinServiceConfig {
    fn from(v: BitcoinConfig) -> Self {
        let ip = v.docker_host.unwrap_or(String::from("127.0.0.1"));
        Self {
            node_url: format!("{}:{}", ip, v.rpc_port),
            node_username: v.rpc_user,
            node_password: v.rpc_password,
            network: v.network,
            da_private_key: None,
            tx_backup_dir: String::new(),
            monitoring: Default::default(),
        }
    }
}

/// A configuration type to define the behaviour of the pruner.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PruningConfig {
    /// Defines the number of blocks from the tip of the chain to remove.
    pub distance: u64,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self { distance: 256 }
    }
}
