use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Boundless storage configuration for S3
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BoundlessS3StorageConfig {
    /// S3 access key
    pub s3_access_key: String,
    /// S3 secret key
    pub s3_secret_key: String,
    /// S3 bucket
    pub s3_bucket: String,
    /// S3 URL
    pub s3_url: String,
    /// S3 region
    pub aws_region: String,
    /// Use presigned URLs for S3
    pub s3_use_presigned: bool,
}

/// Boundless storage configuration for Pinata
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BoundlessPinataStorageConfig {
    /// Pinata JWT for authentication
    pub pinata_jwt: String,
    /// Pinata API URL
    pub pinata_api_url: String,
    /// IPFS Gateway URL
    pub ipfs_gateway_url: String,
}

/// Boundless storage configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BoundlessStorageConfig {
    /// S3 storage provider
    S3(BoundlessS3StorageConfig),
    /// Pinata storage provider
    Pinata(BoundlessPinataStorageConfig),
}

/// Configuration for the Boundless prover
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BoundlessProverConfig {
    /// Boundless configuration
    pub boundless: BoundlessConfig,
    /// Storage configuration
    pub storage: BoundlessStorageConfig,
    /// Pricing service configuration
    pub pricing_service: PricingServiceConfig,
}

/// Configuration for the boundless pricing service
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PricingServiceConfig {
    /// Base URL for the pricing service API
    pub base_url: String,
    /// HTTP client timeout in seconds
    #[serde(default = "default_pricing_service_timeout")]
    pub timeout_secs: u64,
}

#[inline]
const fn default_pricing_service_timeout() -> u64 {
    30
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
/// Configuration for the Boundless Market client
pub struct BoundlessConfig {
    pub wallet_private_key: String,
    pub rpc_url: String,
    pub is_offchain: bool,
}

/// Configuration for the local (IPC) prover
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct LocalProverConfig {
    /// Optional path to the r0vm binary
    pub r0vm_path: Option<PathBuf>,
    /// Enable dev mode
    #[serde(default)]
    pub dev_mode: bool,
}

/// Configuration for the Bonsai prover
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BonsaiProverConfig {
    /// Bonsai API URL
    pub api_url: String,
    /// Bonsai API key
    pub api_key: String,
}

/// Prover configuration enum
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Risc0ProverConfig {
    /// Local IPC prover
    Local(LocalProverConfig),
    /// Bonsai remote prover
    Bonsai(BonsaiProverConfig),
    /// Boundless market prover
    Boundless(Box<BoundlessProverConfig>),
}

impl Default for Risc0ProverConfig {
    fn default() -> Self {
        Self::Local(LocalProverConfig::default())
    }
}

/// Configuration for Risc0Host
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Risc0HostConfig {
    /// Prover config
    pub prover: Risc0ProverConfig,
    /// Optional backup directory for transaction data
    pub tx_backup_dir: Option<PathBuf>,
}
