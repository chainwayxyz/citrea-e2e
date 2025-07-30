use std::path::PathBuf;

use bitcoin::Network;
use tempfile::Builder;

use crate::{log_provider::LogPathProvider, node::NodeKind};

#[derive(Debug, Clone)]
pub struct BitcoinConfig {
    pub p2p_port: u16,
    pub rpc_port: u16,
    pub rpc_user: String,
    pub rpc_password: String,
    pub data_dir: PathBuf,
    pub extra_args: Vec<&'static str>,
    pub network: Network,
    pub docker_image: Option<String>,
    pub env: Vec<(&'static str, &'static str)>,
    pub idx: usize,
    pub docker_host: Option<String>,
}

impl Default for BitcoinConfig {
    fn default() -> Self {
        Self {
            p2p_port: 0,
            rpc_port: 0,
            rpc_user: "user".to_string(),
            rpc_password: "password".to_string(),
            data_dir: Builder::new()
                .keep(true)
                .tempdir()
                .expect("Failed to create temporary directory")
                .path()
                .to_path_buf(),
            extra_args: Vec::new(),
            network: Network::Regtest,
            docker_image: None,
            env: Vec::new(),
            idx: 0,
            docker_host: None,
        }
    }
}

impl BitcoinConfig {
    fn base_args(&self) -> Vec<String> {
        vec![
            "-regtest".to_string(),
            format!("-datadir={}", self.data_dir.display()),
            format!("-port={}", self.p2p_port),
            format!("-rpcport={}", self.rpc_port),
            format!("-rpcuser={}", self.rpc_user),
            format!("-rpcpassword={}", self.rpc_password),
            "-server".to_string(),
            "-daemonwait".to_string(),
            "-txindex".to_string(),
            "-onlynet=ipv4".to_string(),
            "-addresstype=bech32m".to_string(),
            "-debug=net".to_string(),
            "-debug=rpc".to_string(),
            // Required for clementine
            "-fallbackfee=0.00001".to_string(),
            "-dustrelayfee=0".to_string(),
        ]
    }

    pub fn args(&self) -> Vec<String> {
        [
            self.base_args(),
            self.extra_args.iter().map(|&s| s.to_string()).collect(),
        ]
        .concat()
    }

    /// Args to use whe running local bitcoind node
    /// This prevents odd port conflict when assigning rpc/p2p ports
    pub fn local_args(&self) -> Vec<String> {
        [
            self.args(),
            vec![
                format!("-bind=0.0.0.0:{}", self.p2p_port),
                format!("-bind=0.0.0.0:{}", self.rpc_port),
            ],
        ]
        .concat()
    }
}

impl LogPathProvider for BitcoinConfig {
    fn kind(&self) -> NodeKind {
        NodeKind::Bitcoin
    }

    fn log_path(&self) -> PathBuf {
        self.data_dir.join("regtest").join("debug.log")
    }

    fn stderr_path(&self) -> PathBuf {
        self.data_dir.join("stderr.log")
    }
}
