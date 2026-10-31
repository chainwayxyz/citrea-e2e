//! Configuration for SP1 proving.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Configuration for SP1Host.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SP1HostConfig {
    /// Optional backup directory for proof inputs.
    pub tx_backup_dir: Option<PathBuf>,
}
