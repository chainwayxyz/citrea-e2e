use std::time::Duration;

use anyhow::Ok;
use async_trait::async_trait;
use citrea_e2e::{
    config::{TestCaseConfig, TestCaseDockerConfig},
    framework::TestFramework,
    test_case::{TestCase, TestCaseRunner},
    Result,
};

struct ClementineIntegrationTest;

#[async_trait]
impl TestCase for ClementineIntegrationTest {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            with_clementine: true,
            n_verifiers: 2,
            n_operators: 2,
            with_full_node: true,
            docker: TestCaseDockerConfig {
                bitcoin: true,
                citrea: true,
                clementine: false,
            },
            ..Default::default()
        }
    }

    async fn run_test(&mut self, _f: &mut TestFramework) -> Result<()> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        Ok(())
    }
}

#[tokio::test]
async fn test_clementine_integration() -> Result<()> {
    TestCaseRunner::new(ClementineIntegrationTest).run().await
}
