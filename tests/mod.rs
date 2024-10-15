use async_trait::async_trait;
use bitcoincore_rpc::RpcApi;
use citrea_e2e::bitcoin::FINALITY_DEPTH;
use citrea_e2e::config::{TestCaseConfig, TestCaseDockerConfig};
use citrea_e2e::framework::TestFramework;
use citrea_e2e::test_case::{TestCase, TestCaseRunner};
use citrea_e2e::Result;

struct DockerIntegrationTest;

#[async_trait]
impl TestCase for DockerIntegrationTest {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            with_batch_prover: true,
            with_full_node: true,
            docker: TestCaseDockerConfig {
                bitcoin: true,
                citrea: true,
            },
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> Result<()> {
        let sequencer = f.sequencer.as_ref().unwrap();
        let batch_prover = f.batch_prover.as_ref().unwrap();
        let full_node = f.full_node.as_ref().unwrap();
        let da = f.bitcoin_nodes.get(0).unwrap();

        let min_soft_confirmations_per_commitment =
            sequencer.min_soft_confirmations_per_commitment();

        for _ in 0..min_soft_confirmations_per_commitment {
            sequencer.client.send_publish_batch_request().await?;
        }

        da.generate(FINALITY_DEPTH, None).await?;

        // Wait for blob inscribe tx to be in mempool
        da.wait_mempool_len(1, None).await?;

        da.generate(FINALITY_DEPTH, None).await?;
        let finalized_height = da.get_finalized_height().await?;
        batch_prover
            .wait_for_l1_height(finalized_height, None)
            .await?;

        let finalized_height = da.get_finalized_height().await?;
        da.generate(FINALITY_DEPTH, None).await?;

        let commitments = full_node
            .wait_for_sequencer_commitments(finalized_height, None)
            .await?;

        assert_eq!(commitments.len(), 1);

        Ok(())
    }
}

#[tokio::test]
async fn test_docker_integration() -> Result<()> {
    TestCaseRunner::new(DockerIntegrationTest).run().await
}
