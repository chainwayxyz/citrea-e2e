use std::path::PathBuf;

use bitcoin::Network;
use citrea_config::BitcoinServiceConfig;
use tempfile::TempDir;

use crate::{log_provider::LogPathProvider, node::NodeKind};

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
            data_dir: TempDir::new()
                .expect("Failed to create temporary directory")
                .into_path(),
            extra_args: Vec::new(),
            network: Network::Regtest,
            docker_image: Some("bitcoin/bitcoin:28.0".to_string()),
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
            "-addresstype=bech32m".to_string(),
            "-debug=net".to_string(),
            "-debug=rpc".to_string(),
        ]
    }

    pub fn args(&self) -> Vec<String> {
        [
            self.base_args(),
            self.extra_args.iter().map(|&s| s.to_string()).collect(),
        ]
        .concat()
    }
}

impl LogPathProvider for BitcoinConfig {
    fn kind() -> NodeKind {
        NodeKind::Bitcoin
    }

    fn log_path(&self) -> PathBuf {
        self.data_dir.join("regtest").join("debug.log")
    }

    fn stderr_path(&self) -> PathBuf {
        self.data_dir.join("stderr.log")
    }
}
