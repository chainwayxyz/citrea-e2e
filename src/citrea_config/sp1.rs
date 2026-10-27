//! Configuration for SP1 proving.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The SP1 prover backend. Mock proving is for development only.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SP1ProverConfig {
    /// CPU proving on this machine.
    #[default]
    Cpu,
    /// Succinct's prover network, authenticated by NETWORK_PRIVATE_KEY.
    Network,
    /// Execute the guest and return an explicitly unverified development proof.
    Mock,
}

/// Configuration for SP1Host.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SP1HostConfig {
    /// Prover backend.
    #[serde(default)]
    pub prover: SP1ProverConfig,
    /// Optional backup directory for proof inputs.
    pub tx_backup_dir: Option<PathBuf>,
    /// Succinct prover network request policy.
    #[serde(default)]
    pub network: SP1NetworkConfig,
}

/// How network proof requests are resubmitted. Unset durations use the SDK defaults.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SP1NetworkConfig {
    /// Highest price offered, as a percentage of the network's suggested price per
    /// prover gas unit. A request no prover bids on is resubmitted at twice the
    /// previous percentage, up to this cap.
    pub max_price_buffer_pct: u64,
    /// Seconds to wait for a prover to bid before the request is resubmitted.
    pub auction_timeout_secs: Option<u64>,
    /// Seconds a claimed request may take before it is resubmitted.
    pub timeout_secs: Option<u64>,
}

impl Default for SP1NetworkConfig {
    fn default() -> Self {
        Self {
            max_price_buffer_pct: 480,
            auction_timeout_secs: None,
            timeout_secs: None,
        }
    }
}
