use anyhow::bail;
use async_trait::async_trait;

use citrea_e2e::{
    config::TestCaseConfig,
    framework::TestFramework,
    test_case::{TestCase, TestCaseRunner},
};

struct BasicClementineTest;

#[async_trait]
impl TestCase for BasicClementineTest {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            with_verifier: true,
            with_sequencer: false,
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> citrea_e2e::Result<()> {
        let Some(_da) = f.bitcoin_nodes.get(0) else {
            bail!("bitcoind not running!")
        };
        let Some(_verifier) = &f.verifier else {
            bail!("Verifier is not running!")
        };

        Ok(())
    }
}

#[tokio::test]
async fn basic_clementine_test() -> citrea_e2e::Result<()> {
    TestCaseRunner::new(BasicClementineTest).run().await
}
