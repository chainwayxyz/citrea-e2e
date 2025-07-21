use crate::{
    config::{
        AggregatorConfig, ClementineClusterConfig, ClementineConfig, OperatorConfig, VerifierConfig,
    },
    docker::DockerEnv,
    traits::{NodeT, SpawnOutput},
    utils::{get_clementine_path, get_workspace_root, wait_for_tcp_bound},
    Result,
};
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use futures::future::try_join_all;
use std::{
    collections::HashMap,
    fmt::Debug,
    fs::File,
    process::Stdio,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

pub const CLEMENTINE_NODE_STARTUP_TIMEOUT: Duration = Duration::from_secs(360);

pub struct ClementineAggregator {
    pub config: ClementineConfig<AggregatorConfig>,
    spawn_output: SpawnOutput,
    pub client: (), // Simple unit client for now
}

impl ClementineAggregator {
    pub async fn new(
        config: &ClementineConfig<AggregatorConfig>,
        docker: Arc<Option<DockerEnv>>,
    ) -> Result<Self> {
        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;

        let instance = Self {
            config: config.clone(),
            spawn_output,
            client: (),
        };

        instance
            .wait_for_ready(Some(CLEMENTINE_NODE_STARTUP_TIMEOUT))
            .await?;
        debug!("Started Clementine aggregator");

        Ok(instance)
    }
}

#[async_trait]
impl NodeT for ClementineAggregator {
    type Config = ClementineConfig<AggregatorConfig>;
    type Client = ();

    async fn spawn(config: &Self::Config, _docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        let mut env_vars = get_common_env_vars(config);

        // Aggregator specific configuration
        let verifier_endpoints = config.entity_config.verifier_endpoints.join(",");
        let operator_endpoints = config.entity_config.operator_endpoints.join(",");
        env_vars.insert("VERIFIER_ENDPOINTS".to_string(), verifier_endpoints);
        env_vars.insert("OPERATOR_ENDPOINTS".to_string(), operator_endpoints);
        env_vars.insert(
            "SECRET_KEY".to_string(),
            "3333333333333333333333333333333333333333333333333333333333333333".to_string(),
        );

        // Aggregator uses port 8082 for telemetry
        if config.telemetry.is_some() {
            env_vars.insert("TELEMETRY_PORT".to_string(), "8082".to_string());
        }

        spawn_clementine_node(config, "aggregator", None, env_vars).await
    }

    fn spawn_output(&mut self) -> &mut SpawnOutput {
        &mut self.spawn_output
    }

    fn config_mut(&mut self) -> &mut Self::Config {
        &mut self.config
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }

    async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<()> {
        wait_for_tcp_bound("127.0.0.1", self.config.port, timeout)
            .await
            .context("Clementine aggregator failed to become ready")
    }

    fn client(&self) -> &Self::Client {
        &self.client
    }
}

pub struct ClementineVerifier {
    pub config: ClementineConfig<VerifierConfig>,
    spawn_output: SpawnOutput,
    pub client: (), // Simple unit client for now
    index: u8,
}

impl ClementineVerifier {
    pub async fn new(
        config: &ClementineConfig<VerifierConfig>,
        _docker: Arc<Option<DockerEnv>>,
        index: u8,
    ) -> Result<Self> {
        let mut env_vars = get_common_env_vars(config);
        env_vars.insert(
            "SECRET_KEY".to_string(),
            hex::encode(config.entity_config.secret_key.secret_bytes()),
        );

        let spawn_output = spawn_clementine_node(config, "verifier", Some(index), env_vars).await?;

        let instance = Self {
            config: config.clone(),
            spawn_output,
            client: (),
            index,
        };

        instance
            .wait_for_ready(Some(CLEMENTINE_NODE_STARTUP_TIMEOUT))
            .await?;
        debug!("Started Clementine verifier {}", index);

        Ok(instance)
    }
}

#[async_trait]
impl NodeT for ClementineVerifier {
    type Config = ClementineConfig<VerifierConfig>;
    type Client = ();

    async fn spawn(config: &Self::Config, _docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        let mut env_vars = get_common_env_vars(config);
        env_vars.insert(
            "SECRET_KEY".to_string(),
            hex::encode(config.entity_config.secret_key.secret_bytes()),
        );

        spawn_clementine_node(config, "verifier", None, env_vars).await
    }

    fn spawn_output(&mut self) -> &mut SpawnOutput {
        &mut self.spawn_output
    }

    fn config_mut(&mut self) -> &mut Self::Config {
        &mut self.config
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }

    async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<()> {
        wait_for_tcp_bound("127.0.0.1", self.config.port, timeout)
            .await
            .context(format!(
                "Clementine verifier {} failed to become ready",
                self.index
            ))
    }

    fn client(&self) -> &Self::Client {
        &self.client
    }
}

pub struct ClementineOperator {
    pub config: ClementineConfig<OperatorConfig>,
    spawn_output: SpawnOutput,
    pub client: (), // Simple unit client for now
    index: u8,
}

impl ClementineOperator {
    pub async fn new(
        config: &ClementineConfig<OperatorConfig>,
        _docker: Arc<Option<DockerEnv>>,
        index: u8,
    ) -> Result<Self> {
        let mut env_vars = get_common_env_vars(config);

        // Operator specific configuration
        env_vars.insert(
            "SECRET_KEY".to_string(),
            hex::encode(config.entity_config.secret_key.secret_bytes()),
        );
        env_vars.insert(
            "WINTERNITZ_SECRET_KEY".to_string(),
            hex::encode(config.entity_config.winternitz_secret_key.secret_bytes()),
        );
        env_vars.insert(
            "OPERATOR_WITHDRAWAL_FEE_SATS".to_string(),
            config
                .entity_config
                .operator_withdrawal_fee_sats
                .to_sat()
                .to_string(),
        );

        if let Some(addr) = &config.entity_config.operator_reimbursement_address {
            env_vars.insert(
                "OPERATOR_REIMBURSEMENT_ADDRESS".to_string(),
                addr.clone().assume_checked().to_string(),
            );
        }

        if let Some(outpoint) = &config.entity_config.operator_collateral_funding_outpoint {
            env_vars.insert(
                "OPERATOR_COLLATERAL_FUNDING_OUTPOINT".to_string(),
                format!("{}:{}", outpoint.txid, outpoint.vout),
            );
        }

        let spawn_output = spawn_clementine_node(config, "operator", Some(index), env_vars).await?;

        let instance = Self {
            config: config.clone(),
            spawn_output,
            client: (),
            index,
        };

        instance
            .wait_for_ready(Some(CLEMENTINE_NODE_STARTUP_TIMEOUT))
            .await?;
        debug!("Started Clementine operator {}", index);

        Ok(instance)
    }
}

#[async_trait]
impl NodeT for ClementineOperator {
    type Config = ClementineConfig<OperatorConfig>;
    type Client = ();

    async fn spawn(config: &Self::Config, _docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        let mut env_vars = get_common_env_vars(config);

        // Operator specific configuration
        env_vars.insert(
            "SECRET_KEY".to_string(),
            hex::encode(config.entity_config.secret_key.secret_bytes()),
        );
        env_vars.insert(
            "WINTERNITZ_SECRET_KEY".to_string(),
            hex::encode(config.entity_config.winternitz_secret_key.secret_bytes()),
        );
        env_vars.insert(
            "OPERATOR_WITHDRAWAL_FEE_SATS".to_string(),
            config
                .entity_config
                .operator_withdrawal_fee_sats
                .to_sat()
                .to_string(),
        );

        if let Some(addr) = &config.entity_config.operator_reimbursement_address {
            env_vars.insert(
                "OPERATOR_REIMBURSEMENT_ADDRESS".to_string(),
                addr.clone().assume_checked().to_string(),
            );
        }

        if let Some(outpoint) = &config.entity_config.operator_collateral_funding_outpoint {
            env_vars.insert(
                "OPERATOR_COLLATERAL_FUNDING_OUTPOINT".to_string(),
                format!("{}:{}", outpoint.txid, outpoint.vout),
            );
        }

        spawn_clementine_node(config, "operator", None, env_vars).await
    }

    fn spawn_output(&mut self) -> &mut SpawnOutput {
        &mut self.spawn_output
    }

    fn config_mut(&mut self) -> &mut Self::Config {
        &mut self.config
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }

    async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<()> {
        wait_for_tcp_bound("127.0.0.1", self.config.port, timeout)
            .await
            .with_context(|| format!("Clementine operator {} failed to become ready", self.index))
    }

    fn client(&self) -> &Self::Client {
        &self.client
    }
}

pub struct ClementineCluster {
    pub aggregator: ClementineAggregator,
    pub verifiers: Vec<ClementineVerifier>,
    pub operators: Vec<ClementineOperator>,
}

impl ClementineCluster {
    pub async fn new(
        config: &ClementineClusterConfig,
        docker: Arc<Option<DockerEnv>>,
    ) -> Result<Self> {
        generate_certs_if_needed().await?;

        // Setup databases for clementine nodes
        setup_clementine_databases(&config.verifiers, &config.operators).await?;

        let mut verifiers = Vec::new();
        for (index, verifier_config) in config.verifiers.iter().enumerate() {
            verifiers.push(ClementineVerifier::new(
                verifier_config,
                Arc::clone(&docker),
                index as u8,
            ));
        }

        let mut operators = Vec::new();
        for (index, operator_config) in config.operators.iter().enumerate() {
            operators.push(ClementineOperator::new(
                operator_config,
                Arc::clone(&docker),
                index as u8,
            ));
        }

        let verifiers = try_join_all(verifiers.into_iter()).await?;
        let operators = try_join_all(operators.into_iter()).await?;

        // Start aggregator last (similar to run.sh delay)
        debug!("Starting aggregator after verifiers and operators...");
        let aggregator = ClementineAggregator::new(&config.aggregator, Arc::clone(&docker)).await?;

        Ok(Self {
            aggregator,
            verifiers,
            operators,
        })
    }

    pub async fn stop_all(&mut self) -> Result<()> {
        self.aggregator.stop().await?;

        for verifier in &mut self.verifiers {
            verifier.stop().await?;
        }

        for operator in &mut self.operators {
            operator.stop().await?;
        }

        Ok(())
    }
}

/// Ensures that TLS certificates exist for tests.
/// This will run the certificate generation script if certificates don't exist.
pub async fn generate_certs_if_needed() -> std::result::Result<(), std::io::Error> {
    // avoids double generation of certs when multiple tests run in parallel
    static GENERATE_LOCK: LazyLock<tokio::sync::Mutex<()>> =
        LazyLock::new(|| tokio::sync::Mutex::new(()));

    if !get_workspace_root()
        .join("resources/clementine/certs/ca/ca.pem")
        .exists()
    {
        let _lock = GENERATE_LOCK.lock().await;

        debug!("Generating TLS certificates for tests...");

        let script_path = get_workspace_root().join("resources/clementine/generate-certs.sh");

        let output = Command::new("sh").arg(script_path).output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to generate certificates: {}", stderr);
            return Err(std::io::Error::other(format!(
                "Certificate generation failed: {}",
                stderr
            )));
        }
    }

    Ok(())
}

/// Shared function to spawn any Clementine node type
async fn spawn_clementine_node<E: Debug + Clone>(
    config: &ClementineConfig<E>,
    role: &str,
    index: Option<u8>,
    mut env_vars: HashMap<String, String>,
) -> Result<SpawnOutput> {
    let binary_path = get_clementine_path()?;

    if std::env::var("RISC0_DEV_MODE") != Ok("1".to_string()) && cfg!(target_arch = "aarch64") {
        warn!("Spawning Clementine {role} without dev mode in arm64, likely to crash");
    }

    debug!("Spawning Clementine {} with config {:?}", role, config);

    // Create log directory if it doesn't exist
    tokio::fs::create_dir_all(&config.log_dir)
        .await
        .context("Failed to create log directory")?;

    // Generate log file names
    let log_filename = match index {
        Some(idx) => format!("{}-{}.log", role, idx),
        None => format!("{}.log", role),
    };
    let stderr_filename = match index {
        Some(idx) => format!("{}-{}.stderr", role, idx),
        None => format!("{}.stderr", role),
    };

    let log_path = config.log_dir.join(&log_filename);
    let stderr_path = config.log_dir.join(&stderr_filename);

    let stdout_file = File::create(&log_path).context("Failed to create stdout file")?;

    info!("{} stdout logs available at: {}", role, log_path.display());

    let stderr_file = File::create(&stderr_path).context("Failed to create stderr file")?;

    // Add Rust runtime environment variables if they exist
    for var in &["RUSTFLAGS", "CARGO_LLVM_COV", "LLVM_PROFILE_FILE"] {
        if let Ok(val) = std::env::var(var) {
            env_vars.insert(var.to_string(), val);
        }
    }

    // Build command arguments
    let mut args = vec![role.to_string()];

    // Add protocol paramset if specified
    if let Some(paramset_path) = &config.protocol_paramset {
        args = vec![
            "--protocol-params".to_string(),
            paramset_path.display().to_string(),
            role.to_string(),
        ];
    }

    let child = Command::new(&binary_path)
        .args(args)
        .envs(env_vars)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .with_context(|| format!("Failed to spawn Clementine {} process", role))?;

    Ok(SpawnOutput::Child(child))
}

/// Common environment variables shared by all Clementine node types
fn get_common_env_vars<E: Debug + Clone>(config: &ClementineConfig<E>) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Basic environment setup
    env.insert("READ_CONFIG_FROM_ENV".to_string(), "1".to_string());
    env.insert("HOST".to_string(), config.host.clone());
    env.insert("PORT".to_string(), config.port.to_string());
    env.insert("DB_HOST".to_string(), config.db_host.clone());
    env.insert("DB_PORT".to_string(), config.db_port.to_string());
    env.insert("DB_USER".to_string(), config.db_user.clone());
    env.insert("DB_PASSWORD".to_string(), config.db_password.clone());
    env.insert("DB_NAME".to_string(), config.db_name.clone());

    // Bitcoin configuration
    env.insert(
        "BITCOIN_RPC_URL".to_string(),
        config.bitcoin_rpc_url.clone(),
    );
    env.insert(
        "BITCOIN_RPC_USER".to_string(),
        config.bitcoin_rpc_user.clone(),
    );
    env.insert(
        "BITCOIN_RPC_PASSWORD".to_string(),
        config.bitcoin_rpc_password.clone(),
    );

    // Citrea configuration
    env.insert("CITREA_RPC_URL".to_string(), config.citrea_rpc_url.clone());
    env.insert(
        "CITREA_LIGHT_CLIENT_PROVER_URL".to_string(),
        config.citrea_light_client_prover_url.clone(),
    );
    env.insert(
        "CITREA_CHAIN_ID".to_string(),
        config.citrea_chain_id.to_string(),
    );
    env.insert(
        "BRIDGE_CONTRACT_ADDRESS".to_string(),
        config.bridge_contract_address.clone(),
    );

    // TLS configuration
    env.insert(
        "CA_CERT_PATH".to_string(),
        config.ca_cert_path.display().to_string(),
    );
    env.insert(
        "SERVER_CERT_PATH".to_string(),
        config.server_cert_path.display().to_string(),
    );
    env.insert(
        "SERVER_KEY_PATH".to_string(),
        config.server_key_path.display().to_string(),
    );
    env.insert(
        "CLIENT_CERT_PATH".to_string(),
        config.client_cert_path.display().to_string(),
    );
    env.insert(
        "CLIENT_KEY_PATH".to_string(),
        config.client_key_path.display().to_string(),
    );
    env.insert(
        "AGGREGATOR_CERT_PATH".to_string(),
        config.aggregator_cert_path.display().to_string(),
    );
    env.insert(
        "CLIENT_VERIFICATION".to_string(),
        if config.client_verification {
            "1".to_string()
        } else {
            "0".to_string()
        },
    );

    // Security council
    env.insert(
        "SECURITY_COUNCIL".to_string(),
        config.security_council.to_string(),
    );

    // Telemetry (default configuration, may be overridden by specific node types)
    if let Some(telemetry) = &config.telemetry {
        env.insert("TELEMETRY_HOST".to_string(), telemetry.host.clone());
        env.insert("TELEMETRY_PORT".to_string(), telemetry.port.to_string());
    }

    env
}

async fn setup_clementine_databases(
    verifiers: &[ClementineConfig<VerifierConfig>],
    operators: &[ClementineConfig<OperatorConfig>],
) -> Result<()> {
    debug!("Setting up Clementine databases...");

    // Collect all unique database names
    let mut db_names = std::collections::HashSet::new();

    for verifier in verifiers {
        db_names.insert(&verifier.db_name);
    }

    for operator in operators {
        db_names.insert(&operator.db_name);
    }

    // Get Postgres connection info from the first verifier config
    if let Some(first_verifier) = verifiers.first() {
        let db_user = &first_verifier.db_user;
        let db_host = &first_verifier.db_host;
        let db_port = first_verifier.db_port;

        // Set environment variables for Postgres tools
        std::env::set_var("PGUSER", db_user);
        std::env::set_var("PGPASSWORD", &first_verifier.db_password);
        std::env::set_var("PGHOST", db_host);
        std::env::set_var("PGPORT", db_port.to_string());

        // Drop and recreate databases
        for db_name in &db_names {
            debug!("Dropping database: {}", db_name);
            let _ = tokio::process::Command::new("dropdb")
                .arg(db_name)
                .output()
                .await; // Ignore errors as database might not exist

            debug!("Creating database: {}", db_name);
            let output = tokio::process::Command::new("createdb")
                .args(["-O", db_user, db_name])
                .output()
                .await
                .context("Failed to execute createdb")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("Failed to create database {}: {}", db_name, stderr));
            }
        }
    }

    debug!("Successfully set up Clementine databases");
    Ok(())
}
