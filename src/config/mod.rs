mod bitcoin;
#[cfg(feature = "clementine")]
mod clementine;
mod docker;
#[cfg(feature = "clementine")]
mod postgres;
mod test;
mod test_case;
mod throttle;
mod utils;

use std::{
    fmt::{self, Debug},
    path::PathBuf,
};

pub use bitcoin::BitcoinConfig;
#[cfg(feature = "clementine")]
pub use clementine::{
    AggregatorConfig, ClementineClusterConfig, ClementineConfig, ClementineEntityConfig,
    OperatorConfig, VerifierConfig,
};
pub use docker::{DockerConfig, VolumeConfig};
#[cfg(feature = "clementine")]
pub(crate) use postgres::PostgresConfig;
use serde::Serialize;
pub use test::TestConfig;
pub use test_case::{TestCaseConfig, TestCaseDockerConfig, TestCaseEnv};
pub use throttle::ThrottleConfig;
pub use utils::config_to_file;

pub use crate::citrea_config::{
    batch_prover::{BatchProverConfig, ProverGuestRunConfig},
    bitcoin::BitcoinServiceConfig,
    light_client_prover::LightClientProverConfig,
    risc0::{
        BonsaiProverConfig, BoundlessConfig, BoundlessPinataStorageConfig, BoundlessProverConfig,
        BoundlessS3StorageConfig, BoundlessStorageConfig, LocalProverConfig, PricingServiceConfig,
        Risc0HostConfig, Risc0ProverConfig,
    },
    rollup::{RollupConfig, RollupPublicKeys, RpcConfig, RunnerConfig, StorageConfig},
    sequencer::{ListenModeConfig, SequencerConfig, SequencerMempoolConfig},
};
use crate::{log_provider::LogPathProvider, node::NodeKind, Result};

#[derive(Clone, Debug, Default)]
pub enum DaLayer {
    #[default]
    Bitcoin,
    MockDa,
}

impl fmt::Display for DaLayer {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DaLayer::Bitcoin => write!(f, "bitcoin"),
            DaLayer::MockDa => write!(f, "mock"),
        }
    }
}

#[derive(Clone, Debug, Default, Copy)]
pub enum CitreaMode {
    #[default]
    Dev,
    DevAllForks,
}

impl fmt::Display for CitreaMode {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CitreaMode::Dev => write!(f, "dev"),
            CitreaMode::DevAllForks => write!(f, "dev-all-forks"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BaseNodeConfig {
    pub dir: PathBuf,
    pub env: Vec<(&'static str, &'static str)>,
    pub da_layer: DaLayer,
    pub docker_image: Option<String>,
    pub mode: CitreaMode,
}

#[derive(Clone, Debug)]
pub struct FullL2NodeConfig<T>
where
    T: Serialize,
{
    pub base: BaseNodeConfig,
    pub node: T,
    pub rollup: RollupConfig,
    kind: NodeKind,
    pub throttle: Option<ThrottleConfig>,
}

impl<T> FullL2NodeConfig<T>
where
    T: Clone + Serialize + Debug + Send + Sync,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: NodeKind,
        node: T,
        rollup: RollupConfig,
        docker_image: Option<String>,
        dir: PathBuf,
        env: Vec<(&'static str, &'static str)>,
        mode: CitreaMode,
        throttle: Option<ThrottleConfig>,
    ) -> Result<Self> {
        let base = BaseNodeConfig {
            dir: dir.clone(),
            env,
            da_layer: DaLayer::Bitcoin,
            docker_image,
            mode,
        };

        let conf = Self {
            base,
            node,
            rollup,
            kind,
            throttle,
        };

        conf.write_to_file()?;

        Ok(conf)
    }

    // Write configs to files
    pub fn write_to_file(&self) -> Result<()> {
        let dir = &self.base.dir;
        if let Some(config) = self.node_config() {
            let config_path = dir.join(format!("{}_config.toml", self.kind));
            config_to_file(config, &config_path)?;
        }

        let rollup_path = dir.join(format!("{}_rollup_config.toml", self.kind));
        config_to_file(&self.rollup, &rollup_path)?;

        Ok(())
    }
}

pub type FullSequencerConfig = FullL2NodeConfig<SequencerConfig>;
pub type FullBatchProverConfig = FullL2NodeConfig<BatchProverConfig>;
pub type FullLightClientProverConfig = FullL2NodeConfig<LightClientProverConfig>;

#[derive(Serialize, Clone, Debug)]
pub struct EmptyConfig;

pub type FullFullNodeConfig = FullL2NodeConfig<EmptyConfig>;

impl<T> FullL2NodeConfig<T>
where
    T: Clone + Serialize + Debug + Send + Sync,
{
    pub fn dir(&self) -> &PathBuf {
        &self.base.dir
    }

    pub fn set_dir(&mut self, new_dir: PathBuf) {
        self.base.dir = new_dir
    }

    pub fn rpc_bind_host(&self) -> &str {
        &self.rollup.rpc.bind_host
    }

    pub fn rpc_bind_port(&self) -> u16 {
        self.rollup.rpc.bind_port
    }

    pub fn env(&self) -> Vec<(&'static str, &'static str)> {
        self.base.env.clone()
    }

    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    pub fn node_config(&self) -> Option<&T> {
        if std::mem::size_of::<T>() == 0 {
            None
        } else {
            Some(&self.node)
        }
    }
    pub fn rollup_config(&self) -> &RollupConfig {
        &self.rollup
    }

    // Get node config path argument and path.
    // Not required for `full-node`
    pub fn get_node_config_args(&self) -> Option<Vec<String>> {
        let dir = self.dir();
        let kind = self.kind();
        self.node_config().map(|_| {
            let config_path = dir.join(format!("{kind}_config.toml"));
            let node_kind_str = kind.to_string();
            vec![
                format!("--{node_kind_str}"),
                config_path.display().to_string(),
            ]
        })
    }

    // Get rollup config path argument and path.
    pub fn get_rollup_config_args(&self) -> Vec<String> {
        let kind = self.kind();
        vec![
            format!("--rollup-config-path"),
            self.base
                .dir
                .join(format!("{kind}_rollup_config.toml"))
                .display()
                .to_string(),
        ]
    }

    pub fn da_layer(&self) -> &DaLayer {
        &self.base.da_layer
    }

    pub fn mode(&self) -> &CitreaMode {
        &self.base.mode
    }

    /// Normalizes docker-network URLs to host-local URLs for spawned binaries
    pub fn normalize_network_for_local_process(&mut self) {
        self.rollup.da.node_url = rewrite_url_host_to_localhost(&self.rollup.da.node_url);

        if let Some(runner) = self.rollup.runner.as_mut() {
            runner.sequencer_client_url =
                rewrite_url_host_to_localhost(&runner.sequencer_client_url);
        }
    }
}

impl<T> LogPathProvider for FullL2NodeConfig<T>
where
    T: Clone + Serialize + Debug + Send + Sync,
{
    fn kind(&self) -> NodeKind {
        self.kind()
    }

    fn log_path(&self) -> PathBuf {
        self.dir().join("stdout.log")
    }

    fn stderr_path(&self) -> PathBuf {
        self.dir().join("stderr.log")
    }
}

fn rewrite_url_host_to_localhost(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };

    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let suffix = &rest[authority_end..];

    let (userinfo, host_port) = match authority.rsplit_once('@') {
        Some((userinfo, host_port)) => (Some(userinfo), host_port),
        None => (None, authority),
    };

    let mut rewritten = format!("{scheme}://");
    if let Some(userinfo) = userinfo {
        rewritten.push_str(userinfo);
        rewritten.push('@');
    }

    rewritten.push_str(&localhost_with_port(host_port));
    rewritten.push_str(suffix);
    rewritten
}

fn localhost_with_port(host_port: &str) -> String {
    if host_port.is_empty() {
        return "127.0.0.1".to_string();
    }

    let port = if host_port.starts_with('[') {
        host_port
            .find(']')
            .and_then(|idx| host_port.get(idx + 1..))
            .and_then(|rest| rest.strip_prefix(':'))
    } else {
        host_port.rsplit_once(':').map(|(_, port)| port)
    };

    match port.filter(|port| !port.is_empty()) {
        Some(port) => format!("127.0.0.1:{port}"),
        None => "127.0.0.1".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::rewrite_url_host_to_localhost;

    #[test]
    fn rewrite_url_preserves_path_and_port() {
        let url = "http://bitcoin-123.e2e.internal:18443/wallet/sequencer";
        let expected = "http://127.0.0.1:18443/wallet/sequencer";
        assert_eq!(rewrite_url_host_to_localhost(url), expected);
    }

    #[test]
    fn rewrite_url_preserves_userinfo_query_and_fragment() {
        let url = "https://user:pass@bitcoin-123.e2e.internal:18443/path?x=1#tip";
        let expected = "https://user:pass@127.0.0.1:18443/path?x=1#tip";
        assert_eq!(rewrite_url_host_to_localhost(url), expected);
    }

    #[test]
    fn rewrite_url_without_scheme_is_left_unchanged() {
        let url = "bitcoin-123.e2e.internal:18443/wallet/sequencer";
        assert_eq!(rewrite_url_host_to_localhost(url), url);
    }
}
