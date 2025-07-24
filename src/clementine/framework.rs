//! Framework integration utilities for Clementine
//!
//! This module provides clean integration between the test framework and Clementine,
//! handling configuration generation, resource management, and initialization.

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::{config::TestCaseConfig, test_case::TestCase, Result};

/// Framework integration utilities for Clementine
pub struct FrameworkIntegration;

impl FrameworkIntegration {
    /// Initialize Clementine certificates if needed
    pub async fn init_certificates() -> Result<()> {
        super::generate_certs_if_needed().await.map_err(Into::into)
    }

    /// Copy Clementine resources to target directory
    pub fn copy_resources(
        clementine_dir: &Option<String>,
        target_dir: &Path,
    ) -> std::io::Result<()> {
        use crate::utils::{copy_directory, get_workspace_root};

        let clementine_dir = clementine_dir.as_ref().map_or_else(
            || get_workspace_root().join("resources/clementine"),
            PathBuf::from,
        );
        copy_directory(clementine_dir, target_dir)
    }

    /// Generate Clementine cluster configuration
    pub fn generate_cluster_config<T: TestCase>(
        test_case: &TestCaseConfig,
        clementine_dir: &Path,
        postgres: crate::config::PostgresConfig,
        bitcoin_config: crate::config::BitcoinConfig,
        full_node_rpc: crate::config::RpcConfig,
        light_client_rpc: crate::config::RpcConfig,
    ) -> Result<crate::config::ClementineClusterConfig> {
        use crate::{
            config::{
                AggregatorConfig, ClementineClusterConfig, ClementineConfig, OperatorConfig,
                VerifierConfig,
            },
            utils::get_available_port,
        };

        let clementine_logs_dir = clementine_dir.join("logs");
        std::fs::create_dir_all(&clementine_logs_dir).with_context(|| {
            format!(
                "Failed to create {} directory",
                clementine_logs_dir.display()
            )
        })?;

        let mut verifiers = vec![];
        let mut verifier_endpoints = vec![];
        for i in 0..test_case.n_verifiers {
            let port = get_available_port()?;
            verifiers.push(ClementineConfig::<VerifierConfig>::new(
                i,
                postgres.clone(),
                bitcoin_config.clone(),
                full_node_rpc.clone(),
                light_client_rpc.clone(),
                clementine_dir.to_path_buf(),
                port,
                T::clementine_verifier_config(i),
            ));
            verifier_endpoints.push(format!("https://127.0.0.1:{}", port));
        }

        let mut operators = vec![];
        let mut operator_endpoints = vec![];
        for i in 0..test_case.n_operators {
            let port = get_available_port()?;
            operators.push(ClementineConfig::<OperatorConfig>::new(
                i,
                postgres.clone(),
                bitcoin_config.clone(),
                full_node_rpc.clone(),
                light_client_rpc.clone(),
                clementine_dir.to_path_buf(),
                port,
                T::clementine_operator_config(i),
            ));
            operator_endpoints.push(format!("https://127.0.0.1:{}", port));
        }

        let port = get_available_port()?;
        let aggregator = ClementineConfig::<AggregatorConfig>::new(
            verifier_endpoints,
            operator_endpoints,
            postgres.clone(),
            bitcoin_config.clone(),
            full_node_rpc.clone(),
            light_client_rpc.clone(),
            clementine_dir.to_path_buf(),
            port,
            T::clementine_aggregator_config(),
        );

        Ok(ClementineClusterConfig {
            aggregator,
            operators,
            verifiers,
        })
    }
}

/// Stubs for when Clementine feature is disabled
#[cfg(not(feature = "clementine"))]
pub mod stubs {
    use super::*;

    /// Framework integration stubs for when Clementine is disabled
    pub struct FrameworkIntegrationStub;

    impl FrameworkIntegrationStub {
        /// No-op when clementine is disabled
        pub async fn init_certificates() -> Result<()> {
            Ok(())
        }

        /// No-op when clementine is disabled
        pub fn copy_resources(
            _clementine_dir: &Option<String>,
            _target_dir: &Path,
        ) -> std::io::Result<()> {
            Ok(())
        }

        /// Return default stub config when clementine is disabled
        pub fn generate_cluster_config<T: TestCase>(
            _test_case: &TestCaseConfig,
            _clementine_dir: &Path,
            _postgres: crate::config::PostgresConfig,
            _bitcoin_config: crate::config::BitcoinConfig,
            _full_node_rpc: crate::config::RpcConfig,
            _light_client_rpc: crate::config::RpcConfig,
        ) -> Result<crate::config::StubClementineClusterConfig> {
            Ok(Default::default())
        }
    }
}
