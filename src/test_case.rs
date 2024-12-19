//! This module provides the `TestCaseRunner` and `TestCase` trait for running and defining test cases.
//! It handles setup, execution, and cleanup of test environments.

use std::{
    io::Write,
    marker::PhantomData,
    panic::{self},
    path::Path,
    time::Duration,
};

use anyhow::{bail, Context};
use async_trait::async_trait;
use futures::FutureExt;
use tokio::signal;

use super::{
    config::{BitcoinConfig, TestCaseConfig, TestCaseEnv},
    framework::TestFramework,
    Result,
};
use crate::{config::ConfigBounds, traits::NodeT};

const CITREA_ENV: &str = "CITREA_E2E_TEST_BINARY";
const BITCOIN_ENV: &str = "BITCOIN_E2E_TEST_BINARY";
const CLEMENTINE_ENV: &str = "CLEMENTINE_E2E_TEST_BINARY";

// TestCaseRunner manages the lifecycle of a test case, including setup, execution, and cleanup.
/// It creates a test framework with the associated configs, spawns required nodes, connects them,
/// runs the test case, and performs cleanup afterwards. The `run` method handles any panics that
/// might occur during test execution and takes care of cleaning up and stopping the child processes.
pub struct TestCaseRunner<T: TestCase<S, BP, LP>, S, BP, LP>
where
    S: ConfigBounds,
    BP: ConfigBounds,
    LP: ConfigBounds,
{
    test_case: T,
    _marker: PhantomData<(S, BP, LP)>,
}

impl<T: TestCase<S, BP, LP>, S, BP, LP> TestCaseRunner<T, S, BP, LP>
where
    S: ConfigBounds,
    BP: ConfigBounds,
    LP: ConfigBounds,
{
    /// Creates a new `TestCaseRunner`` with the given test case.
    pub fn new(test_case: T) -> Self {
        Self {
            test_case,
            _marker: PhantomData,
        }
    }

    /// Internal method to fund the wallets, connect the nodes, wait for them to be ready.
    async fn prepare(&self, f: &mut TestFramework<S, BP, LP>) -> Result<()> {
        f.fund_da_wallets().await?;
        f.init_nodes().await?;
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

    async fn run_test_case(&mut self, f: &mut TestFramework<S, BP, LP>) -> Result<()> {
        self.prepare(f).await?;
        self.test_case.setup(f).await?;
        self.test_case.run_test(f).await
    }

    /// Executes the test case, handling any panics and performing cleanup.
    ///
    /// This sets up the framework, executes the test, and ensures cleanup is performed even if a panic occurs.
    pub async fn run(mut self) -> Result<()> {
        let mut framework = None;

        let result = panic::AssertUnwindSafe(async {
            tokio::select! {
                res = async {
                    framework = Some(TestFramework::new::<T>().await?);
                    let f = framework.as_mut().unwrap();
                    self.run_test_case(f).await
                 } => res,
                _ = signal::ctrl_c() => {
                    println!("Initiating shutdown...");
                    bail!("Shutdown received before completion")
                }
            }
        })
        .catch_unwind()
        .await;

        let f = framework
            .as_mut()
            .with_context(|| format!("Framework not correctly initialized, result {result:?}"))?;

        if std::env::var("DISABLE_DUMP_LOGS").is_err() {
            if let Err(_) | Ok(Err(_)) = result {
                if let Err(e) = f.dump_logs() {
                    eprintln!("Error dumping log: {e}");
                }
            }
        }

        f.stop().await?;

        // Additional test cleanup
        self.test_case.cleanup().await?;

        std::io::stdout().flush()?;
        std::io::stderr().flush()?;

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(panic_error) => panic::resume_unwind(panic_error),
        }
    }

    pub fn set_binary_path<V: AsRef<str>, P: AsRef<Path>>(self, env_var: V, path: P) -> Self {
        std::env::set_var(env_var.as_ref(), path.as_ref().display().to_string());
        self
    }

    /// Sets the path for the Citrea binary in the environment.
    ///
    /// # Arguments
    ///
    /// * `path` - Location of the Citrea binary to be used when spawning binary.
    ///
    pub fn set_citrea_path<P: AsRef<Path>>(self, path: P) -> Self {
        self.set_binary_path(CITREA_ENV, path)
    }

    /// Sets the path for the Bitcoin binary in the environment.
    ///
    /// # Arguments
    ///
    /// * `path` - Location of the Bitcoin binary to be used when spawning binary.
    ///
    pub fn set_bitcoin_path<P: AsRef<Path>>(self, path: P) -> Self {
        self.set_binary_path(BITCOIN_ENV, path)
    }

    /// Sets the path for the Clementine binary in the environment.
    ///
    /// # Arguments
    ///
    /// * `path` - Location of the Clementine binary to be used when spawning binary.
    ///
    pub fn set_clementine_path<P: AsRef<Path>>(self, path: P) -> Self {
        self.set_binary_path(CLEMENTINE_ENV, path)
    }
}

/// Defines the interface for implementing test cases.
///
/// This trait should be implemented by every test case to define the configuration
/// and inner test logic. It provides default configurations that should be sane for most test cases,
/// which can be overridden by implementing the associated methods.
#[async_trait]
pub trait TestCase<S, BP, LP>: Send + Sync + 'static
where
    S: ConfigBounds,
    BP: ConfigBounds,
    LP: ConfigBounds,
{
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
    fn sequencer_config() -> S {
        S::default()
    }

    /// Returns the batch prover configuration for the test.
    /// Override this method to provide a custom batch prover configuration.
    fn batch_prover_config() -> BP {
        BP::default()
    }

    /// Returns the light client prover configuration for the test.
    /// Override this method to provide a custom light client prover configuration.
    fn light_client_prover_config() -> LP {
        LP::default()
    }

    /// Returns the test setup
    /// Override this method to add custom initialization logic
    async fn setup(&self, _framework: &mut TestFramework<S, BP, LP>) -> Result<()> {
        Ok(())
    }

    /// Implements the actual test logic.
    ///
    /// This method is where the test case should be implemented. It receives
    /// a reference to the TestFramework, which provides access to the test environment.
    ///
    /// # Arguments
    /// * `framework` - A reference to the TestFramework instance
    async fn run_test(&mut self, framework: &mut TestFramework<S, BP, LP>) -> Result<()>;

    async fn cleanup(&self) -> Result<()> {
        Ok(())
    }
}
