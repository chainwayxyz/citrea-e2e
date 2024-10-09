use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Runner configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RunnerConfig {
    /// Sequencer client configuration.
    pub sequencer_client_url: String,
    /// Saves sequencer soft confirmations if set to true
    pub include_tx_body: bool,
    /// Only true for tests
    pub accept_public_input_as_proven: Option<bool>,
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
