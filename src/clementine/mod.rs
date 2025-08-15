//! Clementine integration utilities
//!
//! This module provides clean integration between the test framework and Clementine,
//! handling configuration generation, resource management, and initialization.
//!
//! The implementation uses conditional compilation to provide no-op stubs
//! when the clementine feature is disabled.

use std::path::Path;

use crate::Result;
#[cfg(feature = "clementine")]
use crate::{config::TestCaseConfig, test_case::TestCase};

// Import node implementations when clementine feature is enabled
#[cfg(feature = "clementine")]
pub mod client;
#[cfg(feature = "clementine")]
mod nodes;

// Re-export public types and functions when clementine feature is enabled
#[cfg(feature = "clementine")]
pub use nodes::{
    copy_resources, generate_certs_if_needed, ClementineAggregator, ClementineCluster,
    ClementineOperator, ClementineVerifier,
};

/// Clementine integration utilities for the test framework
pub struct ClementineIntegration;

impl ClementineIntegration {
    /// Initialize Clementine certificates if needed
    pub async fn init_certificates() -> Result<()> {
        #[cfg(feature = "clementine")]
        {
            generate_certs_if_needed().await.map_err(Into::into)
        }
        #[cfg(not(feature = "clementine"))]
        {
            Ok(())
        }
    }

    /// Copy Clementine resources to target directory
    pub fn copy_resources(
        clementine_dir: &Option<String>,
        target_dir: &Path,
    ) -> std::io::Result<()> {
        #[cfg(feature = "clementine")]
        {
            copy_resources(clementine_dir, target_dir)
        }
        #[cfg(not(feature = "clementine"))]
        {
            let _ = (clementine_dir, target_dir); // Avoid unused parameter warnings
            Ok(())
        }
    }

    /// Generate Clementine cluster configuration
    /// This method is only available when the clementine feature is enabled
    #[cfg(feature = "clementine")]
    pub fn generate_cluster_config<T: TestCase>(
        test_case: &TestCaseConfig,
        clementine_dir: &Path,
        postgres: crate::config::PostgresConfig,
        bitcoin_config: crate::config::BitcoinConfig,
        full_node_rpc: crate::config::RpcConfig,
        light_client_rpc: crate::config::RpcConfig,
        docker: &Option<crate::docker::DockerEnv>,
    ) -> Result<crate::config::ClementineClusterConfig> {
        use crate::node::NodeKind;
        use anyhow::Context;

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
                docker,
                clementine_dir.to_path_buf(),
                port,
                T::clementine_verifier_config(i),
            ));
            // When running in Docker, use host.docker.internal so containers can reach host-published ports
            let host = docker
                .as_ref()
                .and_then(|d| {
                    d.clementine()
                        .then(|| d.get_hostname(&NodeKind::ClementineVerifier(i)))
                })
                .unwrap_or("127.0.0.1".to_string());
            verifier_endpoints.push(format!("https://{}:{}", host, port));
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
                docker,
                clementine_dir.to_path_buf(),
                port,
                T::clementine_operator_config(i),
            ));
            let host = docker
                .as_ref()
                .and_then(|d| {
                    d.clementine()
                        .then(|| d.get_hostname(&NodeKind::ClementineOperator(i)))
                })
                .unwrap_or("127.0.0.1".to_string());
            operator_endpoints.push(format!("https://{}:{}", host, port));
        }

        let port = get_available_port()?;
        let aggregator = ClementineConfig::<AggregatorConfig>::new(
            verifier_endpoints,
            operator_endpoints,
            postgres.clone(),
            bitcoin_config.clone(),
            full_node_rpc.clone(),
            light_client_rpc.clone(),
            docker,
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

    /// Initialize clementine nodes - only available when clementine feature is enabled
    #[cfg(feature = "clementine")]
    pub async fn init_nodes(
        config: &crate::config::ClementineClusterConfig,
        docker: std::sync::Arc<Option<crate::docker::DockerEnv>>,
        with_clementine: bool,
    ) -> Result<Option<ClementineCluster>> {
        if with_clementine {
            Ok(Some(ClementineCluster::new(config, docker).await?))
        } else {
            Ok(None)
        }
    }

    /// Stop clementine nodes - only available when clementine feature is enabled
    #[cfg(feature = "clementine")]
    pub async fn stop_nodes(clementine_nodes: &mut Option<ClementineCluster>) -> Result<()> {
        if let Some(nodes) = clementine_nodes {
            nodes.stop_all().await?;
            tracing::info!("Successfully stopped clementine nodes");
        }
        Ok(())
    }
}
