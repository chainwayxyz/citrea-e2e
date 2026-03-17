use async_trait::async_trait;
use citrea_e2e::{
    config::{TestCaseConfig, TestCaseDockerConfig},
    framework::TestFramework,
    node::NodeKind,
    test_case::{TestCase, TestCaseRunner},
    traits::NodeT,
    Result,
};
use jsonrpsee::{
    core::client::{ClientT as _, Error as JsonRpcError},
    http_client::HttpClientBuilder,
    rpc_params,
};

struct TxSenderIntegrationTest;

#[async_trait]
impl TestCase for TxSenderIntegrationTest {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            with_sequencer: true,
            with_batch_prover: true,
            docker: TestCaseDockerConfig {
                bitcoin: true,
                citrea: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> Result<()> {
        for owner_kind in [NodeKind::Sequencer, NodeKind::BatchProver] {
            let tx_sender = f.tx_senders.get(&owner_kind).unwrap_or_else(|| {
                panic!("{owner_kind} tx-sender should be available in framework")
            });

            let client = HttpClientBuilder::default().build(tx_sender.client())?;
            let err = client
                .request::<u64, _>("send_tx", rpc_params![])
                .await
                .expect_err("send_tx without params should fail");

            assert!(
                matches!(err, JsonRpcError::Call(_)),
                "expected JSON-RPC response from {owner_kind} tx-sender, got {err}"
            );
        }

        Ok(())
    }
}

#[tokio::test]
async fn test_tx_sender_integration() -> Result<()> {
    TestCaseRunner::new(TxSenderIntegrationTest).run().await
}
