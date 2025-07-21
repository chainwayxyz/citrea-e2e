use std::io::Write;
use std::{path::PathBuf, str::FromStr, sync::LazyLock};

use std::fmt::Debug;

use anyhow::anyhow;
use bitcoin::hashes::{sha256, Hash, HashEngine};
use bitcoin::{
    address::NetworkUnchecked, secp256k1::SecretKey, Address, Amount, OutPoint, XOnlyPublicKey,
};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use crate::config::{BitcoinConfig, PostgresConfig, RpcConfig};
use crate::node::NodeKind;

pub static UNSPENDABLE_XONLY_PUBKEY: LazyLock<bitcoin::secp256k1::XOnlyPublicKey> =
    LazyLock::new(|| {
        XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0")
            .expect("this key is valid")
    });

/// Deterministic random key generation for testing purposes.
fn seeded_key(prefix: &str, idx: u8) -> [u8; 32] {
    let mut hash = sha256::Hash::engine();
    hash.write_all(prefix.as_bytes()).expect("write failed");
    hash.write_all(&[idx]).expect("write failed");
    hash.flush().expect("flush failed");
    hash.midstate().to_byte_array()
}

/// Data structure to represent the security council that can unlock the deposit using an m-of-n multisig to create a replacement deposit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityCouncil {
    pub pks: Vec<XOnlyPublicKey>,
    pub threshold: u32,
}

impl std::fmt::Display for SecurityCouncil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:", self.threshold)?;
        let pks_str = self
            .pks
            .iter()
            .map(|pk| hex::encode(pk.serialize()))
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}", pks_str)
    }
}

impl std::str::FromStr for SecurityCouncil {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(':');
        let threshold_str = parts.next().ok_or_else(|| anyhow!("Missing threshold"))?;
        let pks_str = parts.next().ok_or_else(|| anyhow!("Missing public keys"))?;

        if parts.next().is_some() {
            return Err(anyhow!("Too many parts in security council string"));
        }

        let threshold = threshold_str
            .parse::<u32>()
            .map_err(|e| anyhow!("Invalid threshold: {}", e))?;

        let pks: Result<Vec<XOnlyPublicKey>, _> = pks_str
            .split(',')
            .map(|pk_str| {
                let bytes =
                    hex::decode(pk_str).map_err(|e| anyhow!("Invalid hex in public key: {}", e))?;
                XOnlyPublicKey::from_slice(&bytes).map_err(|e| anyhow!("Invalid public key: {}", e))
            })
            .collect();

        let pks = pks?;

        if pks.is_empty() {
            return Err(anyhow!("No public keys provided"));
        }

        if threshold > pks.len() as u32 {
            return Err(anyhow!(
                "Threshold cannot be greater than number of public keys"
            ));
        }

        Ok(SecurityCouncil { pks, threshold })
    }
}

impl serde::Serialize for SecurityCouncil {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for SecurityCouncil {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelemetryConfig {
    pub host: String,
    pub port: u16,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8081,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AggregatorConfig {
    pub verifier_endpoints: Vec<String>,
    pub operator_endpoints: Vec<String>,
}

impl ClementineConfig<AggregatorConfig> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        verifier_endpoints: Vec<String>,
        operator_endpoints: Vec<String>,
        postgres_config: PostgresConfig,
        bitcoin_config: BitcoinConfig,
        citrea_rpc: RpcConfig,
        citrea_light_client_prover_rpc: RpcConfig,
        clementine_dir: PathBuf,
        port: u16,
        overrides: ClementineConfig<AggregatorConfig>,
    ) -> Self {
        Self {
            port,
            entity_config: AggregatorConfig {
                verifier_endpoints,
                operator_endpoints,
            },
            // Aggregator uses the first verifier's database
            db_name: format!("clementine-{}", 0),
            ..ClementineConfig::<AggregatorConfig>::from_configs(
                postgres_config,
                bitcoin_config,
                citrea_rpc,
                citrea_light_client_prover_rpc,
                clementine_dir,
                overrides,
            )
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OperatorConfig {
    pub secret_key: SecretKey,
    pub winternitz_secret_key: SecretKey,
    pub operator_withdrawal_fee_sats: Amount,
    /// Will be generated by operator if not provided
    pub operator_reimbursement_address: Option<Address<NetworkUnchecked>>,
    /// Will be generated by operator if not provided
    pub operator_collateral_funding_outpoint: Option<OutPoint>,
}

impl Default for OperatorConfig {
    fn default() -> Self {
        Self {
            secret_key: SecretKey::from_str(
                "1111111111111111111111111111111111111111111111111111111111111111",
            )
            .expect("known valid input"),
            winternitz_secret_key: SecretKey::from_str(
                "2222222222222222222222222222222222222222222222222222222222222222",
            )
            .expect("known valid input"),
            operator_withdrawal_fee_sats: Amount::from_sat(100000),
            operator_reimbursement_address: None,
            operator_collateral_funding_outpoint: None,
        }
    }
}

impl OperatorConfig {
    pub fn default_for_idx(idx: u8) -> Self {
        Self {
            secret_key: SecretKey::from_slice(&seeded_key("operator", idx))
                .expect("known valid input"),
            winternitz_secret_key: SecretKey::from_slice(&seeded_key("operator-winternitz", idx))
                .expect("known valid input"),
            ..Default::default()
        }
    }
}

impl ClementineConfig<OperatorConfig> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        idx: u8,
        postgres_config: PostgresConfig,
        bitcoin_config: BitcoinConfig,
        citrea_rpc: RpcConfig,
        citrea_light_client_prover: RpcConfig,
        clementine_dir: PathBuf,
        port: u16,
        overrides: ClementineConfig<OperatorConfig>,
    ) -> Self {
        Self {
            port,
            entity_config: overrides.entity_config.clone(),
            db_name: format!("clementine-{}", idx),
            ..ClementineConfig::<OperatorConfig>::from_configs(
                postgres_config,
                bitcoin_config,
                citrea_rpc,
                citrea_light_client_prover,
                clementine_dir,
                overrides,
            )
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerifierConfig {
    pub secret_key: SecretKey,
}

impl VerifierConfig {
    pub fn default_for_idx(idx: u8) -> Self {
        Self {
            secret_key: SecretKey::from_slice(&seeded_key("verifier", idx))
                .expect("known valid input"),
        }
    }
}

impl Default for VerifierConfig {
    fn default() -> Self {
        Self {
            secret_key: SecretKey::from_str(
                "1111111111111111111111111111111111111111111111111111111111111111",
            )
            .expect("known valid input"),
        }
    }
}

impl ClementineConfig<VerifierConfig> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        idx: u8,
        postgres_config: PostgresConfig,
        bitcoin_config: BitcoinConfig,
        citrea_rpc: RpcConfig,
        citrea_light_client_prover_rpc: RpcConfig,
        clementine_dir: PathBuf,
        port: u16,
        overrides: ClementineConfig<VerifierConfig>,
    ) -> Self {
        Self {
            port,
            entity_config: overrides.entity_config.clone(),
            db_name: format!("clementine-{}", idx),
            ..ClementineConfig::<VerifierConfig>::from_configs(
                postgres_config,
                bitcoin_config,
                citrea_rpc,
                citrea_light_client_prover_rpc,
                clementine_dir,
                overrides,
            )
        }
    }
}
/// Configuration options for any Clementine target (tests, binaries etc.).
/// Named `BridgeConfig` in the original Clementine codebase.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClementineConfig<E: Debug + Clone> {
    // -- Required by all entities --
    /// Protocol paramset
    ///
    /// Sourced from the provided path or uses the default REGTEST paramset in `resources/clementine/regtest_paramset_{standard|nonstandard}.toml`
    #[serde(skip)]
    pub protocol_paramset: Option<PathBuf>,
    /// gRPC bind host of the operator or the verifier
    pub host: String,
    /// gRPC bind port of the operator or the verifier
    pub port: u16,

    /// Bitcoin remote procedure call URL.
    pub bitcoin_rpc_url: String,
    /// Bitcoin RPC user.
    pub bitcoin_rpc_user: String,
    /// Bitcoin RPC user password.
    pub bitcoin_rpc_password: String,
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
    /// Citrea light client prover RPC URL.
    pub citrea_light_client_prover_url: String,
    /// Citrea's EVM Chain ID.
    pub citrea_chain_id: u32,
    /// Bridge contract address.
    pub bridge_contract_address: String,
    // Initial header chain proof receipt's file path.
    pub header_chain_proof_path: Option<PathBuf>,

    /// Security council.
    pub security_council: SecurityCouncil,

    // TLS certificates
    /// Path to the server certificate file.
    ///
    /// Required for all entities.
    pub server_cert_path: PathBuf,
    /// Path to the server key file.
    pub server_key_path: PathBuf,

    /// Path to the client certificate file. (used to communicate with other gRPC services)
    ///
    /// Required for all entities. This is used to authenticate requests.
    /// Aggregator's client certificate should match the expected aggregator
    /// certificate in other entities.
    ///
    /// Aggregator needs this to call other entities, other entities need this
    /// to call their own internal endpoints.
    pub client_cert_path: PathBuf,
    /// Path to the client key file.
    pub client_key_path: PathBuf,

    /// Path to the CA certificate file which is used to verify client
    /// certificates.
    pub ca_cert_path: PathBuf,

    /// Whether client certificates should be restricted to Aggregator and Self certificates.
    ///
    /// Client certificates are always validated against the CA certificate
    /// according to mTLS regardless of this setting.
    pub client_verification: bool,

    /// Path to the aggregator certificate file. (used to authenticate requests from aggregator)
    ///
    /// Aggregator's client cert should be equal to the this certificate.
    pub aggregator_cert_path: PathBuf,

    /// Telemetry configuration
    pub telemetry: Option<TelemetryConfig>,

    /// Entity-specific configuration fields.
    ///
    /// This is not present in the original Clementine codebase, instead all properties are flattened into this struct
    #[serde(flatten)]
    pub entity_config: E,

    /// Logging directory (used in Citrea-E2E code, NOT passed to Clementine)
    pub log_dir: PathBuf,
}

impl<E: Debug + Clone + Default> Default for ClementineConfig<E> {
    fn default() -> Self {
        Self {
            protocol_paramset: None,
            host: "127.0.0.1".to_string(),
            port: 17000,

            bitcoin_rpc_url: "http://127.0.0.1:18443/wallet/admin".to_string(),
            bitcoin_rpc_user: "admin".to_string(),
            bitcoin_rpc_password: "admin".to_string(),

            db_host: "127.0.0.1".to_string(),
            db_port: 5432,
            db_user: "clementine".to_string(),
            db_password: "clementine".to_string(),
            db_name: "clementine".to_string(),

            citrea_rpc_url: "".to_string(),
            citrea_light_client_prover_url: "".to_string(),
            citrea_chain_id: 5655,
            bridge_contract_address: "3100000000000000000000000000000000000002".to_string(),

            header_chain_proof_path: None,

            security_council: SecurityCouncil {
                pks: vec![*UNSPENDABLE_XONLY_PUBKEY],
                threshold: 1,
            },

            server_cert_path: PathBuf::from("certs/server/server.pem"),
            server_key_path: PathBuf::from("certs/server/server.key"),
            client_cert_path: PathBuf::from("certs/client/client.pem"),
            client_key_path: PathBuf::from("certs/client/client.key"),
            ca_cert_path: PathBuf::from("certs/ca/ca.pem"),
            aggregator_cert_path: PathBuf::from("certs/aggregator/aggregator.pem"),
            client_verification: true,

            telemetry: Some(TelemetryConfig::default()),

            entity_config: E::default(),

            log_dir: TempDir::new()
                .expect("Failed to create temporary directory")
                .keep(),
        }
    }
}

impl<E: Debug + Clone + Default + 'static> ClementineConfig<E> {
    /// Uses other configs to generate a ClementineConfig for the given entity type.
    ///
    /// Matches the AggregatorConfig type to determine if the entity is an
    /// aggregator, and selects the appropriate certificate paths.
    ///
    /// overrides: [`Option<ClementineConfig<E>>`] can be used to override some values. Default::default() is used if not provided. For the exact values, check the commented defaults below.
    pub fn from_configs(
        postgres_config: PostgresConfig,
        bitcoin_config: BitcoinConfig,
        citrea_rpc: RpcConfig,
        citrea_light_client_prover_rpc: RpcConfig,
        base_dir: PathBuf,
        overrides: ClementineConfig<E>,
    ) -> Self {
        let is_aggregator =
            std::any::TypeId::of::<E>() == std::any::TypeId::of::<AggregatorConfig>();
        let certificate_base_dir = base_dir.join("certs");

        Self {
            // TODO: need to change the host to 127.0.0.1 until docker support is added
            bitcoin_rpc_url: format!(
                "http://127.0.0.1:{}/wallet/{}",
                bitcoin_config.rpc_port,
                NodeKind::Bitcoin
            ),
            bitcoin_rpc_user: bitcoin_config.rpc_user,
            bitcoin_rpc_password: bitcoin_config.rpc_password,

            db_host: "127.0.0.1".to_string(),
            db_port: postgres_config.port as usize,
            db_user: postgres_config.user,
            db_password: postgres_config.password,
            db_name: "clementine".to_string(), // overriden by caller

            citrea_rpc_url: format!("http://127.0.0.1:{}", citrea_rpc.bind_port),
            citrea_light_client_prover_url: format!(
                "http://127.0.0.1:{}",
                citrea_light_client_prover_rpc.bind_port
            ),

            server_cert_path: certificate_base_dir.join("server").join("server.pem"),
            server_key_path: certificate_base_dir.join("server").join("server.key"),
            client_cert_path: if is_aggregator {
                certificate_base_dir
                    .join("aggregator")
                    .join("aggregator.pem")
            } else {
                certificate_base_dir.join("client").join("client.pem")
            },
            client_key_path: if is_aggregator {
                certificate_base_dir
                    .join("aggregator")
                    .join("aggregator.key")
            } else {
                certificate_base_dir.join("client").join("client.key")
            },
            ca_cert_path: certificate_base_dir.join("ca").join("ca.pem"),
            aggregator_cert_path: certificate_base_dir
                .join("aggregator")
                .join("aggregator.pem"),
            client_verification: !is_aggregator,
            log_dir: base_dir.join("logs"),

            // Manually merge protocol paramset with the overrides.
            protocol_paramset: overrides.protocol_paramset.clone().or_else(|| {
                // default to the base regtest paramset
                Some(base_dir.join("regtest_paramset.toml"))
            }),

            // These values can be overridden by the caller using overrides.
            // header_chain_proof_path: None,
            // security_council: SecurityCouncil {
            //     pks: vec![*UNSPENDABLE_XONLY_PUBKEY],
            //     threshold: 1,
            // },
            // telemetry: None,
            // citrea_chain_id: 5655,
            // bridge_contract_address: "3100000000000000000000000000000000000002".to_string(),
            // entity_config: E::default(),
            ..overrides
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClementineClusterConfig {
    pub aggregator: ClementineConfig<AggregatorConfig>,
    pub operators: Vec<ClementineConfig<OperatorConfig>>,
    pub verifiers: Vec<ClementineConfig<VerifierConfig>>,
}
