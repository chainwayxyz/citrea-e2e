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

impl BoundlessS3StorageConfig {
    pub fn from_env() -> Option<Self> {
        Some(Self {
            s3_access_key: std::env::var("BOUNDLESS_S3_ACCESS_KEY").ok()?,
            s3_secret_key: std::env::var("BOUNDLESS_S3_SECRET_KEY").ok()?,
            s3_bucket: std::env::var("BOUNDLESS_S3_BUCKET").ok()?,
            s3_url: std::env::var("BOUNDLESS_S3_URL").ok()?,
            aws_region: std::env::var("BOUNDLESS_AWS_REGION").ok()?,
            s3_use_presigned: std::env::var("BOUNDLESS_S3_USE_PRESIGNED")
                .map(|s| s.eq_ignore_ascii_case("true") || s == "1")
                .unwrap_or_default(),
        })
    }
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

impl BoundlessPinataStorageConfig {
    pub fn from_env() -> Option<Self> {
        Some(Self {
            pinata_jwt: std::env::var("BOUNDLESS_PINATA_JWT").ok()?,
            pinata_api_url: std::env::var("BOUNDLESS_PINATA_API_URL").ok()?,
            ipfs_gateway_url: std::env::var("BOUNDLESS_IPFS_GATEWAY_URL").ok()?,
        })
    }
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

impl BoundlessProverConfig {
    pub fn from_env() -> Self {
        let boundless = BoundlessConfig::from_env();

        let storage = if let Some(config) = BoundlessS3StorageConfig::from_env() {
            BoundlessStorageConfig::S3(config)
        } else if let Some(config) = BoundlessPinataStorageConfig::from_env() {
            BoundlessStorageConfig::Pinata(config)
        } else {
            panic!("No valid storage configuration found for boundless, provide either S3 or Pinata env vars");
        };

        let pricing_service = PricingServiceConfig::from_env();

        Self {
            boundless,
            storage,
            pricing_service,
        }
    }
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

impl PricingServiceConfig {
    pub fn from_env() -> Self {
        Self {
            base_url: std::env::var("BOUNDLESS_PRICING_SERVICE_URL")
                .expect("BOUNDLESS_PRICING_SERVICE_URL must be set"),
            timeout_secs: std::env::var("BOUNDLESS_PRICING_SERVICE_TIMEOUT_SECS")
                .ok()
                .and_then(|val| val.parse().ok())
                .unwrap_or_else(default_pricing_service_timeout),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
/// Configuration for the Boundless Market client
pub struct BoundlessConfig {
    pub wallet_private_key: String,
    pub rpc_url: String,
    pub is_offchain: bool,
}

impl BoundlessConfig {
    pub fn from_env() -> Self {
        Self {
            wallet_private_key: std::env::var("BOUNDLESS_WALLET_PRIVATE_KEY")
                .expect("BOUNDLESS_WALLET_PRIVATE_KEY must be set"),
            rpc_url: std::env::var("BOUNDLESS_RPC_URL").expect("BOUNDLESS_RPC_URL must be set"),
            is_offchain: std::env::var("BOUNDLESS_IS_OFFCHAIN")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false),
        }
    }
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

impl BonsaiProverConfig {
    pub fn from_env() -> Self {
        Self {
            api_url: std::env::var("BONSAI_API_URL").expect("BONSAI_API_URL must be set"),
            api_key: std::env::var("BONSAI_API_KEY").expect("BONSAI_API_KEY must be set"),
        }
    }
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

impl Risc0ProverConfig {
    pub fn from_env_or_default() -> Self {
        let env_var = std::env::var("RISC0_PROVER");
        println!("RISC0_PROVER env var: {:?}", env_var);
        match env_var.as_deref() {
            Ok("boundless") => {
                println!("Configuring Boundless prover from environment variables");
                Self::Boundless(Box::new(BoundlessProverConfig::from_env()))
            }
            Ok("bonsai") => Self::Bonsai(BonsaiProverConfig::from_env()),
            _ => Self::Local(LocalProverConfig::default()),
        }
    }
}

impl Default for Risc0ProverConfig {
    fn default() -> Self {
        Self::from_env_or_default()
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
