#![cfg(feature = "clementine")]

use async_trait::async_trait;
use citrea_e2e::{
    config::{TestCaseConfig, TestCaseDockerConfig},
    framework::TestFramework,
    test_case::{TestCase, TestCaseRunner},
    Result,
};

/// Integration test for Clementine gRPC clients.
///
/// This test checks that all three Clementine gRPC services are properly callable
/// through their respective test clients.
///
/// ## Tested gRPC Services & Methods:
/// ### Aggregator Service:
/// - `get_nofn_aggregated_xonly_pk()` - Retrieves N-of-N aggregated public key information
/// - `get_entity_statuses(restart_tasks: false)` - Gets statuses of all operators/verifiers
///
/// ### Operator Service (tested for each operator):
/// - `get_x_only_public_key()` - Returns the operator's X-only public key
/// - `get_current_status()` - Provides status of the operator node
///
/// ### Verifier Service:
/// - `get_params()` - Returns verifier parameters including public key
/// - `get_current_status()` - Provides status of the verifier node
struct ClementineIntegrationTest<const WITH_DOCKER: bool>;

#[async_trait]
impl<const WITH_DOCKER: bool> TestCase for ClementineIntegrationTest<WITH_DOCKER> {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            with_clementine: true,
            n_verifiers: 2,
            n_operators: 2,
            with_full_node: true,
            docker: TestCaseDockerConfig {
                bitcoin: true,
                citrea: true,
                clementine: WITH_DOCKER,
            },
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> Result<()> {
        let clementine = f
            .clementine_nodes
            .as_mut()
            .expect("Clementine nodes should be available in framework");

        // Test ClementineAggregator gRPC methods
        let nofn_response = clementine
            .aggregator
            .client
            .get_nofn_aggregated_xonly_pk()
            .await
            .expect("Failed to get N-of-N aggregated xonly public key from aggregator");

        let entity_statuses = clementine
            .aggregator
            .client
            .get_entity_statuses(false)
            .await
            .expect("Failed to get entity statuses from aggregator");

        println!(
            "Aggregator: N-of-N with {} verifiers, {} entities tracked",
            nofn_response.num_verifiers,
            entity_statuses.entity_statuses.len()
        );

        // Test ClementineOperator gRPC methods for all operators
        for (i, operator) in clementine.operators.iter_mut().enumerate() {
            let _xonly_pk = operator
                .client
                .get_x_only_public_key()
                .await
                .unwrap_or_else(|_| panic!("Failed to get public key from operator {}", i));

            let status = operator
                .client
                .get_current_status()
                .await
                .unwrap_or_else(|_| panic!("Failed to get status from operator {}", i));

            println!(
                "Operator {}: automation={}, balance={}",
                i,
                status.automation,
                status.wallet_balance.expect("Balance should be present")
            );
        }

        // Test ClementineVerifier gRPC methods for all verifiers
        for (i, verifier) in clementine.verifiers.iter_mut().enumerate() {
            let _params = verifier
                .client
                .get_params()
                .await
                .unwrap_or_else(|_| panic!("Failed to get params from verifier {}", i));

            let status = verifier
                .client
                .get_current_status()
                .await
                .unwrap_or_else(|_| panic!("Failed to get status from verifier {}", i));

            println!(
                "Verifier {}: automation={}, balance={}",
                i,
                status.automation,
                status.wallet_balance.expect("Balance should be present")
            );
        }

        Ok(())
    }
}

#[tokio::test]
async fn test_clementine_integration_w_docker() -> Result<()> {
    TestCaseRunner::new(ClementineIntegrationTest::<true>)
        .run()
        .await
}

#[tokio::test]
async fn test_clementine_integration_wo_docker() -> Result<()> {
    TestCaseRunner::new(ClementineIntegrationTest::<false>)
        .run()
        .await
}
