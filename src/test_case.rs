//! This module provides the `TestCaseRunner` and `TestCase` trait for running and defining test cases.
//! It handles setup, execution, and cleanup of test environments.

use std::{
    panic::{self},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{bail, Context};
use async_trait::async_trait;
use futures::FutureExt;

use super::{
    config::{
        default_rollup_config, BitcoinConfig, FullBatchProverConfig, FullFullNodeConfig,
        FullSequencerConfig, RollupConfig, TestCaseConfig, TestCaseEnv, TestConfig,
    },
    framework::TestFramework,
    node::NodeKind,
    utils::{copy_directory, get_available_port},
    Result,
};
use crate::{
    config::{
        BitcoinServiceConfig, FullLightClientProverConfig, ProverConfig, RpcConfig, RunnerConfig,
        SequencerConfig, StorageConfig,
    },
    traits::NodeT,
    utils::{get_default_genesis_path, get_workspace_root},
};

// TestCaseRunner manages the lifecycle of a test case, including setup, execution, and cleanup.
/// It creates a test framework with the associated configs, spawns required nodes, connects them,
/// runs the test case, and performs cleanup afterwards. The `run` method handles any panics that
/// might occur during test execution and takes care of cleaning up and stopping the child processes.
pub struct TestCaseRunner<T: TestCase>(T);

impl<T: TestCase> TestCaseRunner<T> {
    /// Creates a new `TestCaseRunner`` with the given test case.
    pub fn new(test_case: T) -> Self {
        Self(test_case)
    }

    /// Internal method to fund the wallets, connect the nodes, wait for them to be ready.
    async fn prepare(&self, f: &mut TestFramework) -> Result<()> {
        f.fund_da_wallets().await?;
        f.init_nodes().await?;
        f.show_log_paths();
        f.bitcoin_nodes.connect_nodes().await?;

        if let Some(sequencer) = &f.sequencer {
            sequencer
                .wait_for_ready(Some(Duration::from_secs(5)))
                .await?;
        }
        if let Some(batch_prover) = &f.batch_prover {
            batch_prover
                .wait_for_ready(Some(Duration::from_secs(5)))
                .await?;
        }
        if let Some(light_client_prover) = &f.light_client_prover {
            light_client_prover
                .wait_for_ready(Some(Duration::from_secs(5)))
                .await?;
        }
        if let Some(full_node) = &f.full_node {
            full_node
                .wait_for_ready(Some(Duration::from_secs(5)))
                .await?;
        }

        Ok(())
    }

    async fn run_test_case(&mut self, f: &mut TestFramework) -> Result<()> {
        self.prepare(f).await?;
        self.0.setup(f).await?;
        self.0.run_test(f).await
    }

    /// Executes the test case, handling any panics and performing cleanup.
    ///
    /// This sets up the framework, executes the test, and ensures cleanup is performed even if a panic occurs.
    pub async fn run(mut self) -> Result<()> {
        let mut framework = None;
        let result = panic::AssertUnwindSafe(async {
            framework = Some(TestFramework::new(Self::generate_test_config()?).await?);
            let f = framework.as_mut().unwrap();
            self.run_test_case(f).await
        })
        .catch_unwind()
        .await;

        let f = framework
            .as_mut()
            .with_context(|| format!("Framework not correctly initialized, result {result:?}"))?;

        if let Err(_) | Ok(Err(_)) = result {
            if let Err(e) = f.dump_log() {
                eprintln!("Error dumping log: {e}");
            }
        }

        f.stop().await?;

        // Additional test cleanup
        self.0.cleanup().await?;

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(panic_error) => {
                let panic_msg = panic_error
                    .downcast_ref::<String>()
                    .map_or_else(|| "Unknown panic".to_string(), ToString::to_string);
                bail!(panic_msg)
            }
        }
    }

    fn generate_test_config() -> Result<TestConfig> {
        let test_case = T::test_config();
        let env = T::test_env();
        let bitcoin = T::bitcoin_config();
        let batch_prover = T::batch_prover_config();
        let light_client_prover = T::light_client_prover_config();
        let sequencer = T::sequencer_config();
        let sequencer_rollup = default_rollup_config();
        let batch_prover_rollup = default_rollup_config();
        let light_client_prover_rollup = default_rollup_config();
        let full_node_rollup = default_rollup_config();

        let [bitcoin_dir, dbs_dir, batch_prover_dir, light_client_prover_dir, sequencer_dir, full_node_dir, genesis_dir, tx_backup_dir] =
            create_dirs(&test_case.dir)?;

        copy_genesis_dir(&test_case.genesis_dir, &genesis_dir)?;

        let mut bitcoin_confs = vec![];
        for i in 0..test_case.n_nodes {
            let data_dir = bitcoin_dir.join(i.to_string());
            std::fs::create_dir_all(&data_dir)
                .with_context(|| format!("Failed to create {} directory", data_dir.display()))?;

            let p2p_port = get_available_port()?;
            let rpc_port = get_available_port()?;

            bitcoin_confs.push(BitcoinConfig {
                p2p_port,
                rpc_port,
                data_dir,
                env: env.bitcoin().clone(),
                idx: i,
                ..bitcoin.clone()
            });
        }

        // Target first bitcoin node as DA for now
        let da_config: BitcoinServiceConfig = bitcoin_confs[0].clone().into();

        let sequencer_rollup = {
            let bind_port = get_available_port()?;
            let node_kind = NodeKind::Sequencer.to_string();
            RollupConfig {
                da: BitcoinServiceConfig {
                    da_private_key: Some(
                        "045FFC81A3C1FDB3AF1359DBF2D114B0B3EFBF7F29CC9C5DA01267AA39D2C78D"
                            .to_string(),
                    ),
                    node_url: format!("http://{}/wallet/{}", da_config.node_url, node_kind),
                    tx_backup_dir: tx_backup_dir.display().to_string(),
                    ..da_config.clone()
                },
                storage: StorageConfig {
                    path: dbs_dir.join(format!("{node_kind}-db")),
                    db_max_open_files: None,
                },
                rpc: RpcConfig {
                    bind_port,
                    ..sequencer_rollup.rpc
                },
                ..sequencer_rollup
            }
        };

        let runner_config = Some(RunnerConfig {
            sequencer_client_url: format!(
                "http://{}:{}",
                sequencer_rollup.rpc.bind_host, sequencer_rollup.rpc.bind_port,
            ),
            include_tx_body: true,
            accept_public_input_as_proven: Some(true),
            sync_blocks_count: 10,
        });

        let batch_prover_rollup = {
            let bind_port = get_available_port()?;
            let node_kind = NodeKind::BatchProver.to_string();
            RollupConfig {
                da: BitcoinServiceConfig {
                    da_private_key: Some(
                        "75BAF964D074594600366E5B111A1DA8F86B2EFE2D22DA51C8D82126A0FCAC72"
                            .to_string(),
                    ),
                    node_url: format!("http://{}/wallet/{}", da_config.node_url, node_kind),
                    tx_backup_dir: tx_backup_dir.display().to_string(),
                    ..da_config.clone()
                },
                storage: StorageConfig {
                    path: dbs_dir.join(format!("{node_kind}-db")),
                    db_max_open_files: None,
                },
                rpc: RpcConfig {
                    bind_port,
                    ..batch_prover_rollup.rpc
                },
                runner: runner_config.clone(),
                ..batch_prover_rollup
            }
        };

        let light_client_prover_rollup = {
            let bind_port = get_available_port()?;
            let node_kind = NodeKind::LightClientProver.to_string();
            RollupConfig {
                da: BitcoinServiceConfig {
                    da_private_key: None,
                    node_url: format!("http://{}/wallet/{}", da_config.node_url, node_kind),
                    tx_backup_dir: tx_backup_dir.display().to_string(),
                    ..da_config.clone()
                },
                storage: StorageConfig {
                    path: dbs_dir.join(format!("{node_kind}-db")),
                    db_max_open_files: None,
                },
                rpc: RpcConfig {
                    bind_port,
                    ..light_client_prover_rollup.rpc
                },
                runner: runner_config.clone(),
                ..light_client_prover_rollup
            }
        };

        let full_node_rollup = {
            let bind_port = get_available_port()?;
            let node_kind = NodeKind::FullNode.to_string();
            RollupConfig {
                da: BitcoinServiceConfig {
                    node_url: format!(
                        "http://{}/wallet/{}",
                        da_config.node_url,
                        NodeKind::Bitcoin // Use default wallet
                    ),
                    tx_backup_dir: tx_backup_dir.display().to_string(),
                    ..da_config.clone()
                },
                storage: StorageConfig {
                    path: dbs_dir.join(format!("{node_kind}-db")),
                    db_max_open_files: None,
                },
                rpc: RpcConfig {
                    bind_port,
                    ..full_node_rollup.rpc
                },
                runner: runner_config.clone(),
                ..full_node_rollup
            }
        };

        Ok(TestConfig {
            bitcoin: bitcoin_confs,
            sequencer: FullSequencerConfig {
                rollup: sequencer_rollup,
                dir: sequencer_dir,
                docker_image: None,
                node: sequencer,
                env: env.sequencer(),
            },
            batch_prover: FullBatchProverConfig {
                rollup: batch_prover_rollup,
                dir: batch_prover_dir,
                docker_image: None,
                node: batch_prover,
                env: env.batch_prover(),
            },
            light_client_prover: FullLightClientProverConfig {
                rollup: light_client_prover_rollup,
                dir: light_client_prover_dir,
                docker_image: None,
                node: light_client_prover,
                env: env.light_client_prover(),
            },
            full_node: FullFullNodeConfig {
                rollup: full_node_rollup,
                dir: full_node_dir,
                docker_image: None,
                node: (),
                env: env.full_node(),
            },
            test_case,
        })
    }
}

/// Defines the interface for implementing test cases.
///
/// This trait should be implemented by every test case to define the configuration
/// and inner test logic. It provides default configurations that should be sane for most test cases,
/// which can be overridden by implementing the associated methods.
#[async_trait]
pub trait TestCase: Send + Sync + 'static {
    /// Returns the test case configuration.
    /// Override this method to provide custom test configurations.
    fn test_config() -> TestCaseConfig {
        TestCaseConfig::default()
    }

    /// Returns the test case env.
    /// Override this method to provide custom env per node.
    fn test_env() -> TestCaseEnv {
        TestCaseEnv::default()
    }

    /// Returns the Bitcoin configuration for the test.
    /// Override this method to provide a custom Bitcoin configuration.
    fn bitcoin_config() -> BitcoinConfig {
        BitcoinConfig::default()
    }

    /// Returns the sequencer configuration for the test.
    /// Override this method to provide a custom sequencer configuration.
    fn sequencer_config() -> SequencerConfig {
        SequencerConfig::default()
    }

    /// Returns the batch prover configuration for the test.
    /// Override this method to provide a custom batch prover configuration.
    fn batch_prover_config() -> ProverConfig {
        ProverConfig::default()
    }

    /// Returns the light client prover configuration for the test.
    /// Override this method to provide a custom light client prover configuration.
    fn light_client_prover_config() -> ProverConfig {
        ProverConfig::default()
    }

    /// Returns the test setup
    /// Override this method to add custom initialization logic
    async fn setup(&self, _framework: &mut TestFramework) -> Result<()> {
        Ok(())
    }

    /// Implements the actual test logic.
    ///
    /// This method is where the test case should be implemented. It receives
    /// a reference to the TestFramework, which provides access to the test environment.
    ///
    /// # Arguments
    /// * `framework` - A reference to the TestFramework instance
    async fn run_test(&mut self, framework: &mut TestFramework) -> Result<()>;

    async fn cleanup(&self) -> Result<()> {
        Ok(())
    }
}

fn create_dirs(base_dir: &Path) -> Result<[PathBuf; 8]> {
    let paths = [
        NodeKind::Bitcoin.to_string(),
        "dbs".to_string(),
        NodeKind::BatchProver.to_string(),
        NodeKind::LightClientProver.to_string(),
        NodeKind::Sequencer.to_string(),
        NodeKind::FullNode.to_string(),
        "genesis".to_string(),
        "inscription_txs".to_string(),
    ]
    .map(|dir| base_dir.join(dir));

    for path in &paths {
        std::fs::create_dir_all(path)
            .with_context(|| format!("Failed to create {} directory", path.display()))?;
    }

    Ok(paths)
}

fn copy_genesis_dir(genesis_dir: &Option<String>, target_dir: &Path) -> std::io::Result<()> {
    let genesis_dir =
        genesis_dir
            .as_ref()
            .map(PathBuf::from)
            .map_or_else(get_default_genesis_path, |dir| {
                if dir.is_absolute() {
                    dir
                } else {
                    get_workspace_root().join(dir)
                }
            });

    copy_directory(genesis_dir, target_dir)
}
