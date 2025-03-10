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
    /// Saves sequencer l2 blocks if set to true
    pub include_tx_body: bool,
    /// Number of blocks to request during sync
    #[serde(default = "default_sync_blocks_count")]
    pub sync_blocks_count: u64,
    /// Configurations for pruning
    pub pruning_config: Option<PruningConfig>,
    /// Scan L1 start height
    #[serde(default = "default_scan_l1_start_height")]
    pub scan_l1_start_height: Option<u64>,
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
const fn default_scan_l1_start_height() -> Option<u64> {
    Some(1)
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
    /// l2 block signing public key of the Sequencer
    #[serde(with = "hex::serde")]
    pub sequencer_public_key: Vec<u8>,
    /// l2 block signing public key of the Sequencer
    #[serde(with = "hex::serde")]
    pub sequencer_k256_public_key: Vec<u8>,
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
pub struct RollupConfig {
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
    /// Telemetry config
    pub telemetry: TelemetryConfig,
}

impl Default for RollupConfig {
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
                monitoring: Some(MonitoringConfig::default()),
            },
            public_keys: RollupPublicKeys {
                sequencer_public_key: vec![
                    32, 64, 64, 227, 100, 193, 15, 43, 236, 156, 31, 229, 0, 161, 205, 76, 36, 124,
                    137, 214, 80, 160, 30, 215, 232, 44, 171, 168, 103, 135, 124, 33,
                ],
                sequencer_k256_public_key: vec![
                    3, 99, 96, 232, 86, 49, 12, 229, 210, 148, 232, 190, 51, 252, 128, 112, 119,
                    220, 86, 172, 128, 217, 93, 156, 212, 221, 189, 33, 50, 94, 255, 115, 247,
                ],
                // private key E9873D79C6D87DC0FB6A5778633389F4453213303DA61F20BD67FC233AA33262
                // Private Key (WIF): 5Kb8kLf9zgWQnogidDA76MzPL6TsZZY36hWXMssSzNydYXYB9KF
                sequencer_da_pub_key: vec![
                    2, 88, 141, 32, 42, 252, 193, 238, 74, 181, 37, 76, 120, 71, 236, 37, 185, 161,
                    53, 187, 218, 15, 43, 198, 158, 225, 167, 20, 116, 159, 215, 125, 201,
                ],
                // private key 56D08C2DDE7F412F80EC99A0A328F76688C904BD4D1435281EFC9270EC8C8707
                // Private Key (WIF): 5JUX9MqyVroDAjP2itrbaenEKNTioGVnnDSYn3PmLgb23TCLWMs
                prover_da_pub_key: vec![
                    3, 238, 218, 184, 136, 228, 95, 59, 220, 62, 201, 145, 140, 73, 28, 17, 229,
                    207, 122, 240, 169, 31, 56, 185, 127, 188, 30, 19, 90, 228, 5, 102, 1,
                ],
            },
            telemetry: Default::default(),
        }
    }
}

/// Telemetry configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TelemetryConfig {
    /// Server host.
    pub bind_host: String,
    /// Server port.
    pub bind_port: u16,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            bind_host: "0.0.0.0".to_owned(),
            bind_port: 0,
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
            monitoring: Some(Default::default()),
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
