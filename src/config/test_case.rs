use std::{env, path::PathBuf, time::Duration};

use tempfile::TempDir;

#[derive(Clone, Default)]
pub struct TestCaseEnv {
    pub test: Vec<(&'static str, &'static str)>,
    pub full_node: Vec<(&'static str, &'static str)>,
    pub sequencer: Vec<(&'static str, &'static str)>,
    pub batch_prover: Vec<(&'static str, &'static str)>,
    pub light_client_prover: Vec<(&'static str, &'static str)>,
    pub bitcoin: Vec<(&'static str, &'static str)>,
}

impl TestCaseEnv {
    // Base env that should apply to every test cases
    fn base_env() -> Vec<(&'static str, &'static str)> {
        vec![("NO_COLOR", "1")]
    }

    fn test_env(&self) -> Vec<(&'static str, &'static str)> {
        [Self::base_env(), self.test.clone()].concat()
    }

    pub fn sequencer(&self) -> Vec<(&'static str, &'static str)> {
        [self.test_env(), self.sequencer.clone()].concat()
    }

    pub fn batch_prover(&self) -> Vec<(&'static str, &'static str)> {
        [self.test_env(), vec![("PARALLEL_PROOF_LIMIT", "1")], self.batch_prover.clone()].concat()
    }

    pub fn light_client_prover(&self) -> Vec<(&'static str, &'static str)> {
        [self.test_env(), self.light_client_prover.clone()].concat()
    }

    pub fn full_node(&self) -> Vec<(&'static str, &'static str)> {
        [self.test_env(), self.full_node.clone()].concat()
    }

    pub fn bitcoin(&self) -> Vec<(&'static str, &'static str)> {
        [self.test_env(), self.bitcoin.clone()].concat()
    }
}

#[derive(Clone, Debug)]
pub struct TestCaseConfig {
    pub n_nodes: usize,
    pub with_sequencer: bool,
    pub with_full_node: bool,
    pub with_batch_prover: bool,
    pub with_light_client_prover: bool,
    pub timeout: Duration,
    pub dir: PathBuf,
    pub docker: TestCaseDockerConfig,
    // Either a relative dir from workspace root, i.e. "./resources/genesis/devnet"
    // Or an absolute path.
    // Defaults to resources/genesis/bitcoin-regtest
    pub genesis_dir: Option<String>,
}

impl Default for TestCaseConfig {
    fn default() -> Self {
        TestCaseConfig {
            n_nodes: 1,
            with_sequencer: true,
            with_batch_prover: false,
            with_light_client_prover: false,
            with_full_node: false,
            timeout: Duration::from_secs(60),
            dir: TempDir::new()
                .expect("Failed to create temporary directory")
                .into_path(),
            docker: TestCaseDockerConfig::default(),
            genesis_dir: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TestCaseDockerConfig {
    pub bitcoin: bool,
    pub citrea: bool,
}

impl Default for TestCaseDockerConfig {
    fn default() -> Self {
        TestCaseDockerConfig {
            bitcoin: parse_bool_env("TEST_BITCOIN_DOCKER"),
            citrea: parse_bool_env("TEST_CITREA_DOCKER"),
        }
    }
}

impl TestCaseDockerConfig {
    pub fn enabled(&self) -> bool {
        self.bitcoin || self.citrea
    }
}

pub fn parse_bool_env(key: &str) -> bool {
    env::var(key)
        .map(|v| &v == "1" || &v.to_lowercase() == "true")
        .unwrap_or(false)
}
