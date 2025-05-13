use std::{env, path::PathBuf, time::Duration};

use tempfile::TempDir;

use super::CitreaMode;
use crate::utils::generate_test_id;

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
        vec![("NO_COLOR", "1"), ("PARALLEL_PROOF_LIMIT", "1")]
    }

    fn test_env(&self) -> Vec<(&'static str, &'static str)> {
        [Self::base_env(), self.test.clone()].concat()
    }

    pub fn sequencer(&self) -> Vec<(&'static str, &'static str)> {
        [self.test_env(), self.sequencer.clone()].concat()
    }

    pub fn batch_prover(&self) -> Vec<(&'static str, &'static str)> {
        [self.test_env(), self.batch_prover.clone()].concat()
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
    pub with_citrea_cli: bool,
    pub timeout: Duration,
    pub dir: PathBuf,
    pub docker: TestCaseDockerConfig,
    // Either a relative dir from workspace root, i.e. "./resources/genesis/devnet"
    // Or an absolute path.
    // Defaults to resources/genesis/bitcoin-regtest
    pub genesis_dir: Option<String>,
    pub test_id: String,
    pub mode: CitreaMode,
}

impl Default for TestCaseConfig {
    fn default() -> Self {
        let test_id = generate_test_id();
        TestCaseConfig {
            n_nodes: 1,
            with_sequencer: true,
            with_batch_prover: false,
            with_light_client_prover: false,
            with_full_node: false,
            with_citrea_cli: false,
            timeout: Duration::from_secs(60),
            dir: std::env::var("TEST_OUT_DIR")
                .map_or_else(
                    |_| {
                        TempDir::new()
                            .expect("Failed to create temporary directory")
                            .keep()
                    },
                    PathBuf::from,
                )
                .join(test_id.clone()),
            docker: TestCaseDockerConfig::default(),
            genesis_dir: None,
            test_id,
            mode: CitreaMode::Dev,
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
            bitcoin: parse_bool_env("TEST_BITCOIN_DOCKER").unwrap_or(true),
            citrea: parse_bool_env("TEST_CITREA_DOCKER").unwrap_or(false),
        }
    }
}

impl TestCaseDockerConfig {
    pub fn enabled(&self) -> bool {
        self.bitcoin || self.citrea
    }
}

pub fn parse_bool_env(key: &str) -> Option<bool> {
    env::var(key)
        .ok()
        .map(|v| &v == "1" || &v.to_lowercase() == "true")
}
