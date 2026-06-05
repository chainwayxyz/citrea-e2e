use std::{collections::HashMap, path::PathBuf};

use anyhow::{bail, Result};
use bitcoin::Network;

use crate::{
    citrea_config::bitcoin::BitcoinServiceConfig,
    config::{BitcoinConfig, PostgresConfig},
    log_provider::LogPathProvider,
    node::NodeKind,
};

#[derive(Clone, Debug)]
pub struct TxSenderConfig {
    pub owner_kind: NodeKind,
    pub rpc_port: u16,
    pub db_host: String,
    pub db_port: u16,
    pub db_user: String,
    pub db_password: String,
    pub db_name: String,
    pub bitcoin_rpc_host: String,
    pub bitcoin_rpc_port: u16,
    pub bitcoin_rpc_path: String,
    pub bitcoin_rpc_user: String,
    pub bitcoin_rpc_password: String,
    pub network: Network,
    pub secret_key: String,
    pub docker_image: Option<String>,
    pub docker_host: Option<String>,
    pub log_dir: PathBuf,
}

pub struct TxSenderConfigInput<'a> {
    pub owner_kind: NodeKind,
    pub postgres_config: &'a PostgresConfig,
    pub bitcoin_config: &'a BitcoinConfig,
    pub da_config: &'a BitcoinServiceConfig,
    pub log_dir: PathBuf,
    pub rpc_port: u16,
    pub docker_host: Option<String>,
    pub test_id: &'a str,
}

impl TxSenderConfig {
    pub fn new(input: TxSenderConfigInput<'_>) -> Result<Self> {
        let Some(secret_key) = input.da_config.da_private_key.clone() else {
            bail!("tx-sender requires a DA private key in the Bitcoin service config");
        };

        Ok(Self {
            owner_kind: input.owner_kind,
            rpc_port: input.rpc_port,
            db_host: input
                .postgres_config
                .docker_host
                .clone()
                .unwrap_or_else(|| "127.0.0.1".to_string()),
            db_port: input.postgres_config.port,
            db_user: input.postgres_config.user.clone(),
            db_password: input.postgres_config.password.clone(),
            db_name: format!(
                "tx_sender_{}_{}",
                input.owner_kind.db_name_component(),
                input.test_id.to_lowercase()
            ),
            bitcoin_rpc_host: input
                .bitcoin_config
                .docker_host
                .clone()
                .unwrap_or_else(|| "127.0.0.1".to_string()),
            bitcoin_rpc_port: input.bitcoin_config.rpc_port,
            bitcoin_rpc_path: bitcoin_rpc_path(&input.da_config.node_url),
            bitcoin_rpc_user: input.da_config.node_username.clone(),
            bitcoin_rpc_password: input.da_config.node_password.clone(),
            network: input.bitcoin_config.network,
            secret_key,
            docker_image: std::env::var("TX_SENDER_DOCKER_IMAGE").ok(),
            docker_host: input.docker_host,
            log_dir: input.log_dir,
        })
    }

    pub fn local_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.rpc_port)
    }

    pub fn url(&self, caller_in_docker: bool) -> String {
        if caller_in_docker {
            self.docker_url()
        } else {
            self.local_url()
        }
    }

    pub fn alias(&self) -> String {
        format!("tx-sender-{}", self.owner_kind)
    }

    pub fn label(&self) -> String {
        self.alias()
    }

    pub fn docker_url(&self) -> String {
        format!(
            "http://{}:{}",
            self.docker_host
                .clone()
                .unwrap_or_else(|| "127.0.0.1".to_string()),
            self.rpc_port
        )
    }

    /// Environment variables for spawning the tx-sender as a local process on the host.
    pub fn env(&self) -> HashMap<String, String> {
        let mut env = self.base_env();
        env.insert(
            "TX_SENDER_JSONRPC_BIND".to_string(),
            "127.0.0.1".to_string(),
        );
        env.insert("DB_HOST".to_string(), "127.0.0.1".to_string());
        env.insert(
            "BITCOIN_RPC_URL".to_string(),
            format!(
                "http://127.0.0.1:{}{}",
                self.bitcoin_rpc_port, self.bitcoin_rpc_path
            ),
        );
        env
    }

    pub fn docker_env(&self) -> HashMap<String, String> {
        let mut env = self.base_env();
        env.insert("TX_SENDER_JSONRPC_BIND".to_string(), "0.0.0.0".to_string());
        env
    }

    fn base_env(&self) -> HashMap<String, String> {
        HashMap::from([
            ("RUST_LOG".to_string(), "info".to_string()),
            ("NETWORK".to_string(), self.network.to_string()),
            ("SECRET_KEY".to_string(), self.secret_key.clone()),
            ("DB_HOST".to_string(), self.db_host.clone()),
            ("DB_PORT".to_string(), self.db_port.to_string()),
            ("DB_USER".to_string(), self.db_user.clone()),
            ("DB_PASSWORD".to_string(), self.db_password.clone()),
            ("DB_NAME".to_string(), self.db_name.clone()),
            (
                "BITCOIN_RPC_URL".to_string(),
                format!(
                    "http://{}:{}{}",
                    self.bitcoin_rpc_host, self.bitcoin_rpc_port, self.bitcoin_rpc_path
                ),
            ),
            (
                "BITCOIN_RPC_USER".to_string(),
                self.bitcoin_rpc_user.clone(),
            ),
            (
                "BITCOIN_RPC_PASSWORD".to_string(),
                self.bitcoin_rpc_password.clone(),
            ),
            (
                "TX_SENDER_NONCE_GRIND_PREFIX".to_string(),
                "[2]".to_string(),
            ),
            (
                "TX_SENDER_JSONRPC_PORT".to_string(),
                self.rpc_port.to_string(),
            ),
            (
                "TX_SENDER_FEE_RATE_HARD_CAP".to_string(),
                "1000".to_string(),
            ),
            (
                "TX_SENDER_MEMPOOL_FEE_RATE_MULTIPLIER".to_string(),
                "1".to_string(),
            ),
            (
                "TX_SENDER_MEMPOOL_FEE_RATE_OFFSET_SAT_KVB".to_string(),
                "0".to_string(),
            ),
            (
                "TX_SENDER_CPFP_FEE_PAYER_BUMP_WAIT_TIME_SECONDS".to_string(),
                "3600".to_string(),
            ),
            (
                "TX_SENDER_FEE_BUMP_AFTER_BLOCKS".to_string(),
                "10".to_string(),
            ),
            ("TX_SENDER_MIN_BUMP_KVB".to_string(), "200".to_string()),
            ("TX_SENDER_FINALITY_DEPTH".to_string(), "5".to_string()),
            ("TX_SENDER_POLL_DELAY_MS".to_string(), "1000".to_string()),
            ("TX_SENDER_INCLUDE_UNSAFE".to_string(), "true".to_string()),
        ])
    }
}

fn bitcoin_rpc_path(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else {
        return String::new();
    };
    let path_start = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    rest[path_start..].to_string()
}

impl LogPathProvider for TxSenderConfig {
    fn kind(&self) -> NodeKind {
        NodeKind::TxSender
    }

    fn log_path(&self) -> PathBuf {
        self.log_dir.join(format!("{}.log", self.alias()))
    }

    fn stderr_path(&self) -> PathBuf {
        self.log_dir.join(format!("{}.stderr", self.alias()))
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::Network;
    use tempfile::TempDir;

    use super::TxSenderConfig;
    use crate::node::NodeKind;

    #[test]
    fn url_uses_docker_host_for_docker_callers() {
        let tempdir = TempDir::new().expect("temp dir");
        let config = TxSenderConfig {
            owner_kind: NodeKind::Sequencer,
            rpc_port: 3030,
            db_host: "127.0.0.1".to_string(),
            db_port: 5432,
            db_user: "postgres".to_string(),
            db_password: "postgres".to_string(),
            db_name: "tx_sender_test".to_string(),
            bitcoin_rpc_host: "127.0.0.1".to_string(),
            bitcoin_rpc_port: 18443,
            bitcoin_rpc_path: "/wallet/sequencer".to_string(),
            bitcoin_rpc_user: "admin".to_string(),
            bitcoin_rpc_password: "admin".to_string(),
            network: Network::Regtest,
            secret_key: "secret".to_string(),
            docker_image: None,
            docker_host: Some("host.docker.internal".to_string()),
            log_dir: tempdir.path().to_path_buf(),
        };

        assert_eq!(config.url(false), "http://127.0.0.1:3030");
        assert_eq!(config.url(true), "http://host.docker.internal:3030");
    }
}
