use std::time::Duration;

use anyhow::bail;
use async_trait::async_trait;
use bitcoincore_rpc::{json::IndexStatus, RpcApi};
use citrea_e2e::{
    config::{BitcoinConfig, TestCaseConfig, TestCaseDockerConfig},
    framework::TestFramework,
    test_case::{TestCase, TestCaseRunner},
    traits::Restart,
    Result,
};

struct BasicSyncTest;

#[async_trait]
impl TestCase for BasicSyncTest {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            with_sequencer: false,
            n_nodes: 2,
            timeout: Duration::from_secs(60),
            docker: TestCaseDockerConfig {
                bitcoin: true,
                citrea: true,
            },
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> Result<()> {
        let (Some(da0), Some(da1)) = (f.bitcoin_nodes.get(0), f.bitcoin_nodes.get(1)) else {
            bail!("bitcoind not running. Test should run with two da nodes")
        };
        let initial_height = f.initial_da_height;

        f.bitcoin_nodes.wait_for_sync(None).await?;
        let height0 = da0.get_block_count().await?;
        let height1 = da1.get_block_count().await?;

        // Assert that nodes are in sync before disconnection
        assert_eq!(height0, height1, "Block heights don't match");

        f.bitcoin_nodes.disconnect_nodes().await?;

        // Generate some blocks on node0
        da0.generate(5).await?;

        let height0 = da0.get_block_count().await?;
        let height1 = da1.get_block_count().await?;

        // Nodes are now out of sync
        assert_eq!(height0, initial_height + 5);
        assert_eq!(height1, initial_height);

        // Reconnect nodes and sync
        f.bitcoin_nodes.connect_nodes().await?;
        f.bitcoin_nodes.wait_for_sync(None).await?;

        let height0 = da0.get_block_count().await?;
        let height1 = da1.get_block_count().await?;

        // Assert that nodes are back in sync
        assert_eq!(height0, height1, "Block heights don't match");

        Ok(())
    }
}

#[tokio::test]
async fn test_basic_sync() -> Result<()> {
    TestCaseRunner::new(BasicSyncTest).run().await
}

struct RestartBitcoinTest;

#[async_trait]
impl TestCase for RestartBitcoinTest {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            with_sequencer: false,
            docker: TestCaseDockerConfig {
                bitcoin: true,
                citrea: true,
            },
            ..Default::default()
        }
    }

    fn bitcoin_config() -> BitcoinConfig {
        BitcoinConfig {
            extra_args: vec!["-txindex=0"],
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> Result<()> {
        let da = f.bitcoin_nodes.get_mut(0).unwrap();
        // Add txindex flag to check that restart takes into account the extra args
        let new_conf = BitcoinConfig {
            extra_args: vec!["-txindex=1"],
            ..da.config.clone()
        };

        let block_before = da.get_block_count().await?;
        let info = da.get_index_info().await?;

        assert_eq!(info.txindex, None);

        // Restart node with txindex
        da.restart(Some(new_conf), None).await?;

        let block_after = da.get_block_count().await?;
        let info = da.get_index_info().await?;

        assert!(matches!(
            info.txindex,
            Some(IndexStatus { synced: true, .. })
        ));
        // Assert that state is kept between restarts
        assert_eq!(block_before, block_after);

        Ok(())
    }
}

#[tokio::test]
async fn test_restart_bitcoin() -> Result<()> {
    TestCaseRunner::new(RestartBitcoinTest).run().await
}
