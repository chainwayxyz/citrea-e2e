use async_trait::async_trait;
use bitcoincore_rpc::RpcApi;
use citrea_e2e::{
    bitcoin::DEFAULT_FINALITY_DEPTH,
    config::{TestCaseConfig, TestCaseDockerConfig},
    framework::TestFramework,
    test_case::{TestCase, TestCaseRunner},
    traits::{Restart, RestartPolicy},
    Result,
};

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
                ..Default::default()
            },
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> Result<()> {
        let sequencer = f.sequencer.as_ref().unwrap();
        let full_node = f.full_node.as_ref().unwrap();
        let batch_prover = f.batch_prover.as_ref().unwrap();
        let da = f.bitcoin_nodes.get(0).unwrap();

        let max_l2_blocks_per_commitment = Self::sequencer_config().max_l2_blocks_per_commitment;

        for _ in 0..max_l2_blocks_per_commitment {
            sequencer.client.send_publish_batch_request().await?;
        }

        // Wait for blob inscribe tx to be in mempool
        da.wait_mempool_len(1, None).await?;

        da.generate(DEFAULT_FINALITY_DEPTH).await?;
        let finalized_height = da.get_finalized_height(None).await?;

        batch_prover
            .wait_for_l1_height(finalized_height, None)
            .await?;
        full_node
            .wait_for_l2_height(max_l2_blocks_per_commitment, None)
            .await?;

        let unspent_sequencer = sequencer
            .da
            .list_unspent(None, None, None, None, None)
            .await?;
        let unspent_da = da.list_unspent(None, None, None, None, None).await?;
        // Make sure sequencer.da and da don't hit the same wallet
        assert_ne!(unspent_sequencer, unspent_da);

        Ok(())
    }
}

#[tokio::test]
async fn test_docker_integration() -> Result<()> {
    TestCaseRunner::new(DockerIntegrationTest).run().await
}

// Test restart in docker
struct DockerSequencerRestartTest;

#[async_trait]
impl TestCase for DockerSequencerRestartTest {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            docker: TestCaseDockerConfig {
                bitcoin: true,
                citrea: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> Result<()> {
        let sequencer = f.sequencer.as_mut().unwrap();

        let height_before = sequencer.client.ledger_get_head_l2_block_height().await?;

        let n_blocks = 3;
        for _ in 0..n_blocks {
            sequencer.client.send_publish_batch_request().await?;
        }

        sequencer
            .wait_for_l2_height(height_before + n_blocks, None)
            .await?;

        let height_pre_restart = sequencer.client.ledger_get_head_l2_block_height().await?;
        sequencer.restart(None, None).await?;
        let height_post_restart = sequencer.client.ledger_get_head_l2_block_height().await?;

        assert_eq!(height_pre_restart, height_post_restart);

        sequencer.client.send_publish_batch_request().await?;
        sequencer
            .wait_for_l2_height(height_post_restart + 1, None)
            .await?;

        Ok(())
    }
}

#[tokio::test]
async fn test_docker_sequencer_restart() -> Result<()> {
    TestCaseRunner::new(DockerSequencerRestartTest).run().await
}

// Test restart from docker to spawned binary
struct DockerSequencerRestartPolicySpawnTest;

#[async_trait]
impl TestCase for DockerSequencerRestartPolicySpawnTest {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            docker: {
                TestCaseDockerConfig {
                    citrea: true, // Start in docker
                    bitcoin: true,
                    ..Default::default()
                }
            },
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> Result<()> {
        let sequencer = f.sequencer.as_mut().unwrap();

        let height_before = sequencer.client.ledger_get_head_l2_block_height().await?;

        let n_blocks = 2;
        for _ in 0..n_blocks {
            sequencer.client.send_publish_batch_request().await?;
        }

        sequencer
            .wait_for_l2_height(height_before + n_blocks, None)
            .await?;

        let height_pre_restart = sequencer.client.ledger_get_head_l2_block_height().await?;

        sequencer.config.restart_policy = RestartPolicy::Spawn; // Switch restart policy to using `CITREA_E2E_TEST_BINARY` binary
        sequencer.restart(None, None).await?;

        let height_post_restart = sequencer.client.ledger_get_head_l2_block_height().await?;

        assert_eq!(height_pre_restart, height_post_restart);

        sequencer.client.send_publish_batch_request().await?;
        sequencer
            .wait_for_l2_height(height_post_restart + 1, None)
            .await?;

        Ok(())
    }
}

#[tokio::test]
async fn test_docker_sequencer_restart_policy_spawn() -> Result<()> {
    TestCaseRunner::new(DockerSequencerRestartPolicySpawnTest)
        .run()
        .await
}
