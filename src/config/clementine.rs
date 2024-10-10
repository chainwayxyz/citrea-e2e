//! # Clementine Configuration Options

use std::path::PathBuf;

use bitcoin::{address::NetworkUnchecked, secp256k1, Amount, Network};
use serde::{Deserialize, Serialize};

/// Clementine's configuration options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClementineClient {
    /// Host of the operator or the verifier
    pub host: String,
    /// Port of the operator or the verifier
    pub port: u16,
    /// Bitcoin network to work on.
    pub network: Network,
    /// Secret key for the operator or the verifier.
    pub secret_key: secp256k1::SecretKey,
    /// Verifiers public keys.
    pub verifiers_public_keys: Vec<secp256k1::PublicKey>,
    /// Number of verifiers.
    pub num_verifiers: usize,
    /// Operators x-only public keys.
    pub operators_xonly_pks: Vec<secp256k1::XOnlyPublicKey>,
    /// Operators wallet addresses.
    pub operator_wallet_addresses: Vec<bitcoin::Address<NetworkUnchecked>>,
    /// Number of operators.
    pub num_operators: usize,
    /// Operator's fee for withdrawal, in satoshis.
    pub operator_withdrawal_fee_sats: Option<Amount>,
    /// Number of blocks after which user can take deposit back if deposit request fails.
    pub user_takes_after: u32,
    /// Number of blocks after which operator can take reimburse the bridge fund if they are honest.
    pub operator_takes_after: u32,
    /// Bridge amount in satoshis.
    pub bridge_amount_sats: Amount,
    /// Operator: number of kickoff UTXOs per funding transaction.
    pub operator_num_kickoff_utxos_per_tx: usize,
    /// Threshold for confirmation.
    pub confirmation_threshold: u32,
    /// Bitcoin remote procedure call URL.
    pub bitcoin_rpc_url: String,
    /// Bitcoin RPC user.
    pub bitcoin_rpc_user: String,
    /// Bitcoin RPC user password.
    pub bitcoin_rpc_password: String,
    /// All Secret keys. Just for testing purposes.
    pub all_verifiers_secret_keys: Option<Vec<secp256k1::SecretKey>>,
    /// All Secret keys. Just for testing purposes.
    pub all_operators_secret_keys: Option<Vec<secp256k1::SecretKey>>,
    /// Verifier endpoints.
    pub verifier_endpoints: Option<Vec<String>>,
    /// PostgreSQL database host address.
    pub db_host: String,
    /// PostgreSQL database port.
    pub db_port: usize,
    /// PostgreSQL database user name.
    pub db_user: String,
    /// PostgreSQL database user password.
    pub db_password: String,
    /// PostgreSQL database name.
    pub db_name: String,
    /// Citrea RPC URL.
    pub citrea_rpc_url: String,
    /// Bridge contract address.
    pub bridge_contract_address: String,
}

impl Default for ClementineClient {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3030,
            secret_key: secp256k1::SecretKey::new(&mut secp256k1::rand::thread_rng()),
            verifiers_public_keys: vec![],
            num_verifiers: 7,
            operators_xonly_pks: vec![],
            operator_wallet_addresses: vec![],
            num_operators: 3,
            operator_withdrawal_fee_sats: None,
            user_takes_after: 5,
            operator_takes_after: 5,
            bridge_amount_sats: Amount::from_sat(100_000_000),
            operator_num_kickoff_utxos_per_tx: 10,
            confirmation_threshold: 1,
            network: Network::Regtest,
            bitcoin_rpc_url: "http://127.0.0.1:18443".to_string(),
            bitcoin_rpc_user: "admin".to_string(),
            bitcoin_rpc_password: "admin".to_string(),
            all_verifiers_secret_keys: None,
            all_operators_secret_keys: None,
            verifier_endpoints: None,
            db_host: "127.0.0.1".to_string(),
            db_port: 5432,
            db_user: "postgres".to_string(),
            db_password: "postgres".to_string(),
            db_name: "postgres".to_string(),
            citrea_rpc_url: "http://127.0.0.1:12345".to_string(),
            bridge_contract_address: "3100000000000000000000000000000000000002".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClementineConfig {
    pub client: ClementineClient,
    pub docker_image: Option<String>,
    pub data_dir: PathBuf,
}

impl Default for ClementineConfig {
    fn default() -> Self {
        Self {
            client: ClementineClient::default(),
            docker_image: None,
            data_dir: PathBuf::from("bridge_backend"),
        }
    }
}
