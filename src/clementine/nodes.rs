use std::{
    collections::HashMap,
    fs::File,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use client::{
    ClementineAggregatorTestClient, ClementineOperatorTestClient, ClementineVerifierTestClient,
    TlsConfig,
};
use futures::future::try_join_all;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use super::client;
use crate::{
    config::{
        AggregatorConfig, ClementineClusterConfig, ClementineConfig, ClementineEntityConfig,
        DockerConfig, OperatorConfig, VerifierConfig, VolumeConfig,
    },
    docker::DockerEnv,
    log_provider::LogPathProvider,
    traits::{NodeT, SpawnOutput},
    utils::{get_clementine_path, get_workspace_root, wait_for_tcp_bound},
    Result,
};

pub const CLEMENTINE_NODE_STARTUP_TIMEOUT: Duration = Duration::from_secs(360);
const DEFAULT_CLEMENTINE_DOCKER_IMAGE: &str =
    "chainwayxyz/clementine@sha256:99111a1d44f7f699d639f104003d301db49739f9926765b2950fae8e448004a9";

pub struct ClementineAggregator {
    pub config: ClementineConfig<AggregatorConfig>,
    spawn_output: SpawnOutput,
    pub client: ClementineAggregatorTestClient,
}

impl ClementineAggregator {
    pub async fn new(
        config: &ClementineConfig<AggregatorConfig>,
        docker: Arc<Option<DockerEnv>>,
    ) -> Result<Self> {
        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;

        // Wait for the gRPC server to be ready
        wait_for_tcp_bound(
            "127.0.0.1",
            config.port,
            Some(CLEMENTINE_NODE_STARTUP_TIMEOUT),
        )
        .await
        .context("Clementine aggregator failed to become ready")?;

        // Create TLS configuration
        let tls_config = TlsConfig::new(
            &config.client_cert_path,
            &config.client_key_path,
            &config.ca_cert_path,
            "localhost".to_string(),
        );

        let endpoint = format!("https://127.0.0.1:{}", config.port);

        let timeout = CLEMENTINE_NODE_STARTUP_TIMEOUT;
        let start = Instant::now();
        let mut result = Err(anyhow!("initial response value"));

        while result.is_err() && (start.elapsed() < timeout) {
            debug!(
                "Aggregator connect attempt after {} seconds",
                start.elapsed().as_secs()
            );
            result =
                ClementineAggregatorTestClient::new(endpoint.clone(), tls_config.clone()).await;

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let client = result.context(format!(
            "Failed to connect to Clementine aggregator in {} seconds",
            start.elapsed().as_secs()
        ))?;

        let instance = Self {
            config: config.clone(),
            spawn_output,
            client,
        };

        debug!("Started Clementine aggregator");
        Ok(instance)
    }
}

#[async_trait]
impl NodeT for ClementineAggregator {
    type Config = ClementineConfig<AggregatorConfig>;
    type Client = ClementineAggregatorTestClient;

    async fn spawn(config: &Self::Config, docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        spawn_clementine_node(config, "aggregator", docker).await
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
    pub client: ClementineVerifierTestClient,
    index: u8,
}

impl ClementineVerifier {
    pub async fn new(
        config: &ClementineConfig<VerifierConfig>,
        docker: Arc<Option<DockerEnv>>,
        index: u8,
    ) -> Result<Self> {
        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;

        // Wait for the gRPC server to be ready
        wait_for_tcp_bound(
            "127.0.0.1",
            config.port,
            Some(CLEMENTINE_NODE_STARTUP_TIMEOUT),
        )
        .await
        .context(format!(
            "Clementine verifier {} failed to become ready",
            index
        ))?;

        // Create TLS configuration using Aggregator client cert for access
        let aggregator_key_path = config.aggregator_cert_path.with_file_name("aggregator.key");
        let tls_config = TlsConfig::new(
            &config.aggregator_cert_path,
            &aggregator_key_path,
            &config.ca_cert_path,
            "localhost".to_string(),
        );
        let endpoint = format!("https://127.0.0.1:{}", config.port);

        let timeout = CLEMENTINE_NODE_STARTUP_TIMEOUT;
        let start = Instant::now();
        let mut result = Err(anyhow!("initial response value"));

        while result.is_err() && (start.elapsed() < timeout) {
            debug!(
                "Verifier connect attempt after {} seconds",
                start.elapsed().as_secs()
            );
            result = ClementineVerifierTestClient::new(endpoint.clone(), tls_config.clone()).await;

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let client = result.context(format!(
            "Failed to connect to Clementine verifier {} in {} seconds",
            index,
            start.elapsed().as_secs()
        ))?;

        let instance = Self {
            config: config.clone(),
            spawn_output,
            client,
            index,
        };

        debug!("Started Clementine verifier {}", index);
        Ok(instance)
    }
}

#[async_trait]
impl NodeT for ClementineVerifier {
    type Config = ClementineConfig<VerifierConfig>;
    type Client = ClementineVerifierTestClient;

    async fn spawn(config: &Self::Config, docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        spawn_clementine_node(config, "verifier", docker).await
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
    pub client: ClementineOperatorTestClient,
    index: u8,
}

impl ClementineOperator {
    pub async fn new(
        config: &ClementineConfig<OperatorConfig>,
        docker: Arc<Option<DockerEnv>>,
        index: u8,
    ) -> Result<Self> {
        let spawn_output = <Self as NodeT>::spawn(config, &docker).await?;

        // Wait for the gRPC server to be ready
        wait_for_tcp_bound(
            "127.0.0.1",
            config.port,
            Some(CLEMENTINE_NODE_STARTUP_TIMEOUT),
        )
        .await
        .with_context(|| format!("Clementine operator {} failed to become ready", index))?;

        // Create TLS configuration
        let tls_config = TlsConfig::new(
            &config.client_cert_path,
            &config.client_key_path,
            &config.ca_cert_path,
            "localhost".to_string(),
        );

        let endpoint = format!("https://127.0.0.1:{}", config.port);

        let timeout = CLEMENTINE_NODE_STARTUP_TIMEOUT;
        let start = Instant::now();
        let mut result = Err(anyhow!("initial response value"));

        while result.is_err() && (start.elapsed() < timeout) {
            debug!(
                "Operator connect attempt after {} seconds",
                start.elapsed().as_secs()
            );
            result = ClementineOperatorTestClient::new(endpoint.clone(), tls_config.clone()).await;

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let client = result.context(format!(
            "Failed to connect to Clementine verifier {} in {} seconds",
            index,
            start.elapsed().as_secs()
        ))?;

        let instance = Self {
            config: config.clone(),
            spawn_output,
            client,
            index,
        };

        debug!("Started Clementine operator {}", index);
        Ok(instance)
    }
}

#[async_trait]
impl NodeT for ClementineOperator {
    type Config = ClementineConfig<OperatorConfig>;
    type Client = ClementineOperatorTestClient;

    async fn spawn(config: &Self::Config, docker: &Arc<Option<DockerEnv>>) -> Result<SpawnOutput> {
        spawn_clementine_node(config, "operator", docker).await
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

/// Copy Clementine resources to target directory
pub fn copy_resources(
    clementine_dir: &Option<String>,
    target_dir: &std::path::Path,
) -> std::io::Result<()> {
    use crate::utils::{copy_directory, get_workspace_root};

    let clementine_dir = clementine_dir.as_ref().map_or_else(
        || get_workspace_root().join("resources/clementine"),
        std::path::PathBuf::from,
    );
    copy_directory(clementine_dir, target_dir)
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
async fn spawn_clementine_node<E: ClementineEntityConfig>(
    config: &ClementineConfig<E>,
    role: &str,
    docker: &Arc<Option<DockerEnv>>,
) -> Result<SpawnOutput>
where
    ClementineConfig<E>: LogPathProvider,
{
    let idx = config.entity_config.idx();

    if std::env::var("RISC0_DEV_MODE") != Ok("1".to_string()) && cfg!(target_arch = "aarch64") {
        warn!("Spawning Clementine {role} without dev mode in arm64, likely to crash");
    }

    debug!("Spawning Clementine {} with config {:?}", role, config);

    // Create directories if they don't exist
    tokio::fs::create_dir_all(&config.log_dir)
        .await
        .context("Failed to create log directory")?;
    tokio::fs::create_dir_all(&config.base_dir.join("configs"))
        .await
        .context("Failed to create base directory")?;

    let log_path = config.log_path();
    let stderr_path = config.stderr_path();
    let stdout_file = File::create(&log_path).context("Failed to create stdout file")?;

    info!("{} stdout logs available at: {}", role, log_path.display());

    let stderr_file = File::create(&stderr_path).context("Failed to create stderr file")?;

    // Add protocol paramset if specified
    let paramset_path = config
        .protocol_paramset
        .as_ref()
        .expect("Expected paramset to be defined here by ClementineConfig");

    let config_path = config
        .base_dir
        .join("configs")
        .join(format!("{role}-{idx}.toml"));

    tokio::fs::write(&config_path, toml::to_string(&config).unwrap()).await?;

    let args = vec![
        "--protocol-params".to_string(),
        paramset_path.display().to_string(),
        "--config".to_string(),
        config_path.display().to_string(),
        role.to_string(),
    ];

    let bitvm_cache_path = {
        let Ok(cache) = std::env::var("BITVM_CACHE_PATH") else {
            anyhow::bail!("BITVM_CACHE_PATH is not set for Clementine {role}");
        };

        let cache_path = PathBuf::from(cache);

        if !matches!(tokio::fs::try_exists(&cache_path).await, Ok(true)) {
            anyhow::bail!("BITVM_CACHE_PATH does not exist: {}", cache_path.display());
        }

        cache_path
    };

    let env = {
        let mut env = HashMap::new();
        // Inherit environment variables from the host process
        for var in &[
            "RUSTFLAGS",
            "CARGO_LLVM_COV",
            "LLVM_PROFILE_FILE",
            "BITVM_CACHE_PATH",
            "RISC0_DEV_MODE",
            "RUST_MIN_STACK",
            "RUST_LOG",
        ] {
            if let Ok(val) = std::env::var(var) {
                env.insert(var.to_string(), val);
            }
        }
        env
    };

    let spawn_output = match docker.as_ref() {
        Some(docker) if docker.clementine() => {
            docker
                .spawn(DockerConfig {
                    ports: vec![config.port],
                    image: config
                        .image
                        .as_deref()
                        .unwrap_or(DEFAULT_CLEMENTINE_DOCKER_IMAGE)
                        .to_string(),
                    cmd: args,
                    host_dir: Some(vec![
                        // Mount the base_dir for config and paramset to be accessible
                        config.base_dir.display().to_string(),
                        // Mount the bitvm cache
                        bitvm_cache_path.display().to_string(),
                    ]),
                    log_path: config.log_path(),
                    volume: VolumeConfig {
                        name: role.to_string(),
                        target: "/not-used".to_string(),
                    },
                    kind: config.kind(),
                    throttle: None,
                    env,
                })
                .await?
        }
        _ => SpawnOutput::Child(
            Command::new(&get_clementine_path()?)
                .args(args)
                .envs(env)
                .stdout(Stdio::from(stdout_file))
                .stderr(Stdio::from(stderr_file))
                .spawn()
                .with_context(|| format!("Failed to spawn Clementine {} process", role))?,
        ),
    };

    Ok(spawn_output)
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
        let db_port = first_verifier.db_port;

        // Set environment variables for Postgres tools
        std::env::set_var("PGUSER", db_user);
        std::env::set_var("PGPASSWORD", &first_verifier.db_password);
        std::env::set_var("PGHOST", "127.0.0.1");
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
