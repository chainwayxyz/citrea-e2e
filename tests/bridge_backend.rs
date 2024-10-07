use anyhow::bail;
use async_trait::async_trait;

use citrea_e2e::{
    config::TestCaseConfig,
    framework::TestFramework,
    test_case::{TestCase, TestCaseRunner},
};

struct BasicBridgeBackendTest;

#[async_trait]
impl TestCase for BasicBridgeBackendTest {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            with_bridge_backend: true,
            with_sequencer: false,
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> citrea_e2e::Result<()> {
        let Some(_da) = f.bitcoin_nodes.get(0) else {
            bail!("bitcoind not running!")
        };
        let Some(_bridge_backend) = f.bridge_backend_nodes.get(0) else {
            bail!("Bridge backend is not running!")
        };

        Ok(())
    }
}

#[tokio::test]
async fn basic_prover_test() -> citrea_e2e::Result<()> {
    TestCaseRunner::new(BasicBridgeBackendTest).run().await
}
