use std::{collections::HashMap, path::PathBuf};

use anyhow::{bail, Result};
use bitcoin::Network;

use crate::{
    citrea_config::bitcoin::BitcoinServiceConfig,
    config::{BitcoinConfig, PostgresConfig},
    log_provider::LogPathProvider,
    node::NodeKind,
};

const DEFAULT_FEE_RATE_HARD_CAP: u64 = 100;
const DEFAULT_MEMPOOL_FEE_RATE_MULTIPLIER: u64 = 1;
const DEFAULT_MEMPOOL_FEE_RATE_OFFSET_SAT_KVB: u64 = 0;
const DEFAULT_CPFP_FEE_PAYER_BUMP_WAIT_TIME_SECONDS: u64 = 3600;
const DEFAULT_FEE_BUMP_AFTER_BLOCKS: u32 = 10;
const DEFAULT_MIN_BUMP_KVB: u64 = 200;
const DEFAULT_FINALITY_DEPTH: u32 = 1;
const DEFAULT_POLL_DELAY_MS: u64 = 30_000;
const DEFAULT_INCLUDE_UNSAFE: bool = true;

#[derive(Clone, Debug)]
pub struct TxSenderConfig {
    pub owner_kind: NodeKind,
    pub rpc_port: u16,
    pub db_host: String,
    pub db_port: u16,
    pub db_user: String,
    pub db_password: String,
    pub db_name: String,
    pub bitcoin_rpc_url: String,
    pub bitcoin_rpc_user: String,
    pub bitcoin_rpc_password: String,
    pub network: Network,
    pub secret_key: String,
    pub private_da_key: Option<String>,
    pub docker_image: Option<String>,
    pub docker_host: Option<String>,
    pub log_dir: PathBuf,
    pub fee_rate_hard_cap: u64,
    pub mempool_fee_rate_multiplier: u64,
    pub mempool_fee_rate_offset_sat_kvb: u64,
    pub cpfp_fee_payer_bump_wait_time_seconds: u64,
    pub fee_bump_after_blocks: u32,
    pub min_bump_kvb: u64,
    pub finality_depth: u32,
    pub poll_delay_ms: u64,
    pub input_unspent_max_retries: Option<u32>,
    pub include_unsafe: bool,
}

impl TxSenderConfig {
    pub fn new(
        owner_kind: NodeKind,
        postgres_config: &PostgresConfig,
        bitcoin_config: &BitcoinConfig,
        da_config: &BitcoinServiceConfig,
        log_dir: PathBuf,
        rpc_port: u16,
        docker_host: Option<String>,
    ) -> Result<Self> {
        let Some(secret_key) = da_config.da_private_key.clone() else {
            bail!("tx-sender requires a DA private key in the Bitcoin service config");
        };

        Ok(Self {
            owner_kind,
            rpc_port,
            db_host: postgres_config
                .docker_host
                .clone()
                .unwrap_or_else(|| "127.0.0.1".to_string()),
            db_port: postgres_config.port,
            db_user: postgres_config.user.clone(),
            db_password: postgres_config.password.clone(),
            db_name: format!("tx_sender_{}", owner_kind.db_name_component()),
            bitcoin_rpc_url: rewrite_bitcoin_rpc_url(
                &da_config.node_url,
                &bitcoin_config
                    .docker_host
                    .clone()
                    .unwrap_or_else(|| "127.0.0.1".to_string()),
                bitcoin_config.rpc_port,
            ),
            bitcoin_rpc_user: da_config.node_username.clone(),
            bitcoin_rpc_password: da_config.node_password.clone(),
            network: bitcoin_config.network,
            secret_key,
            private_da_key: None,
            docker_image: std::env::var("TX_SENDER_DOCKER_IMAGE").ok(),
            docker_host,
            log_dir,
            fee_rate_hard_cap: parse_env_or(
                "TX_SENDER_FEE_RATE_HARD_CAP",
                DEFAULT_FEE_RATE_HARD_CAP,
            )?,
            mempool_fee_rate_multiplier: parse_env_or(
                "TX_SENDER_MEMPOOL_FEE_RATE_MULTIPLIER",
                DEFAULT_MEMPOOL_FEE_RATE_MULTIPLIER,
            )?,
            mempool_fee_rate_offset_sat_kvb: parse_env_or(
                "TX_SENDER_MEMPOOL_FEE_RATE_OFFSET_SAT_KVB",
                DEFAULT_MEMPOOL_FEE_RATE_OFFSET_SAT_KVB,
            )?,
            cpfp_fee_payer_bump_wait_time_seconds: parse_env_or(
                "TX_SENDER_CPFP_FEE_PAYER_BUMP_WAIT_TIME_SECONDS",
                DEFAULT_CPFP_FEE_PAYER_BUMP_WAIT_TIME_SECONDS,
            )?,
            fee_bump_after_blocks: parse_env_or(
                "TX_SENDER_FEE_BUMP_AFTER_BLOCKS",
                DEFAULT_FEE_BUMP_AFTER_BLOCKS,
            )?,
            min_bump_kvb: parse_env_or("TX_SENDER_MIN_BUMP_KVB", DEFAULT_MIN_BUMP_KVB)?,
            finality_depth: parse_env_or("TX_SENDER_FINALITY_DEPTH", DEFAULT_FINALITY_DEPTH)?,
            poll_delay_ms: parse_env_or("TX_SENDER_POLL_DELAY_MS", DEFAULT_POLL_DELAY_MS)?,
            input_unspent_max_retries: parse_optional_env("TX_SENDER_INPUT_UNSPENT_MAX_RETRIES")?,
            include_unsafe: parse_env_or("TX_SENDER_INCLUDE_UNSAFE", DEFAULT_INCLUDE_UNSAFE)?,
        })
    }

    pub fn local_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.rpc_port)
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

    pub fn docker_env(&self) -> HashMap<String, String> {
        let mut env = HashMap::from([
            ("NETWORK".to_string(), self.network.to_string()),
            ("SECRET_KEY".to_string(), self.secret_key.clone()),
            ("DB_HOST".to_string(), self.db_host.clone()),
            ("DB_PORT".to_string(), self.db_port.to_string()),
            ("DB_USER".to_string(), self.db_user.clone()),
            ("DB_PASSWORD".to_string(), self.db_password.clone()),
            ("DB_NAME".to_string(), self.db_name.clone()),
            ("BITCOIN_RPC_URL".to_string(), self.bitcoin_rpc_url.clone()),
            (
                "BITCOIN_RPC_USER".to_string(),
                self.bitcoin_rpc_user.clone(),
            ),
            (
                "BITCOIN_RPC_PASSWORD".to_string(),
                self.bitcoin_rpc_password.clone(),
            ),
            (
                "TX_SENDER_FEE_RATE_HARD_CAP".to_string(),
                self.fee_rate_hard_cap.to_string(),
            ),
            (
                "TX_SENDER_MEMPOOL_FEE_RATE_MULTIPLIER".to_string(),
                self.mempool_fee_rate_multiplier.to_string(),
            ),
            (
                "TX_SENDER_MEMPOOL_FEE_RATE_OFFSET_SAT_KVB".to_string(),
                self.mempool_fee_rate_offset_sat_kvb.to_string(),
            ),
            (
                "TX_SENDER_CPFP_FEE_PAYER_BUMP_WAIT_TIME_SECONDS".to_string(),
                self.cpfp_fee_payer_bump_wait_time_seconds.to_string(),
            ),
            (
                "TX_SENDER_FEE_BUMP_AFTER_BLOCKS".to_string(),
                self.fee_bump_after_blocks.to_string(),
            ),
            (
                "TX_SENDER_MIN_BUMP_KVB".to_string(),
                self.min_bump_kvb.to_string(),
            ),
            (
                "TX_SENDER_FINALITY_DEPTH".to_string(),
                self.finality_depth.to_string(),
            ),
            (
                "TX_SENDER_POLL_DELAY_MS".to_string(),
                self.poll_delay_ms.to_string(),
            ),
            (
                "TX_SENDER_INCLUDE_UNSAFE".to_string(),
                self.include_unsafe.to_string(),
            ),
            ("TX_SENDER_JSONRPC_BIND".to_string(), "0.0.0.0".to_string()),
            (
                "TX_SENDER_JSONRPC_PORT".to_string(),
                self.rpc_port.to_string(),
            ),
        ]);

        if let Some(private_da_key) = &self.private_da_key {
            env.insert("PRIVATE_DA_KEY".to_string(), private_da_key.clone());
        }

        if let Some(max_retries) = self.input_unspent_max_retries {
            env.insert(
                "TX_SENDER_INPUT_UNSPENT_MAX_RETRIES".to_string(),
                max_retries.to_string(),
            );
        }

        env
    }
}

fn rewrite_bitcoin_rpc_url(url: &str, host: &str, port: u16) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return format!("http://{host}:{port}");
    };

    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let suffix = &rest[authority_end..];

    format!("{scheme}://{host}:{port}{suffix}")
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

fn parse_env_or<T: std::str::FromStr>(key: &str, default: T) -> Result<T>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(value) => value
            .parse()
            .map_err(|err| anyhow::anyhow!("Failed to parse {key}={value}: {err}")),
        Err(_) => Ok(default),
    }
}

fn parse_optional_env<T: std::str::FromStr>(key: &str) -> Result<Option<T>>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(value) => value
            .parse()
            .map(Some)
            .map_err(|err| anyhow::anyhow!("Failed to parse {key}={value}: {err}")),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::TxSenderConfig;
    use crate::{
        citrea_config::bitcoin::BitcoinServiceConfig,
        config::{BitcoinConfig, PostgresConfig},
        node::NodeKind,
    };
    use bitcoin::Network;
    use tempfile::TempDir;

    #[test]
    fn docker_env_matches_bitcoin_da_credentials() {
        let tempdir = TempDir::new().expect("temp dir");
        let postgres = PostgresConfig::default();
        let bitcoin = BitcoinConfig {
            rpc_port: 18443,
            rpc_user: "admin".to_string(),
            rpc_password: "admin".to_string(),
            network: Network::Regtest,
            docker_host: Some("bitcoin.test".to_string()),
            ..Default::default()
        };
        let da = BitcoinServiceConfig {
            node_url: "http://127.0.0.1:18443/wallet/sequencer".to_string(),
            node_username: "admin".to_string(),
            node_password: "admin".to_string(),
            network: Network::Regtest,
            da_private_key: Some(
                "E9873D79C6D87DC0FB6A5778633389F4453213303DA61F20BD67FC233AA33262".to_string(),
            ),
            tx_backup_dir: String::new(),
            monitoring: None,
        };

        let config = TxSenderConfig::new(
            NodeKind::Sequencer,
            &postgres,
            &bitcoin,
            &da,
            tempdir.path().to_path_buf(),
            3030,
            Some("tx-sender.test".to_string()),
        )
        .expect("config");

        let env = config.docker_env();
        assert_eq!(
            env.get("SECRET_KEY").map(String::as_str),
            da.da_private_key.as_deref()
        );
        assert_eq!(
            env.get("BITCOIN_RPC_URL").map(String::as_str),
            Some("http://bitcoin.test:18443/wallet/sequencer")
        );
        assert_eq!(
            env.get("BITCOIN_RPC_USER").map(String::as_str),
            Some("admin")
        );
        assert_eq!(
            env.get("BITCOIN_RPC_PASSWORD").map(String::as_str),
            Some("admin")
        );
        assert!(env.get("PRIVATE_DA_KEY").is_none());
    }
}
