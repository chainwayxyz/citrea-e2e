use std::collections::HashMap;

use async_trait::async_trait;
use citrea_e2e::{
    config::{TestCaseConfig, TestCaseDockerConfig},
    framework::TestFramework,
    node::NodeKind,
    test_case::{TestCase, TestCaseRunner},
    Result,
};

struct MutiSequencerTest;

#[async_trait]
impl TestCase for MutiSequencerTest {
    fn test_config() -> TestCaseConfig {
        TestCaseConfig {
            with_sequencer: true,
            n_nodes: HashMap::from([(NodeKind::Sequencer, 2)]),
            docker: TestCaseDockerConfig {
                bitcoin: true,
                citrea: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    async fn run_test(&mut self, f: &mut TestFramework) -> Result<()> {
        let sequencer_cluster = f.sequencer_cluster.as_ref().unwrap();
        let (sequencer0, sequencer1) = (
            sequencer_cluster.get(0).unwrap(),
            sequencer_cluster.get(1).unwrap(),
        );

        let height0 = sequencer0.client.ledger_get_head_l2_block_height().await;
        assert!(height0.is_ok());
        let height1 = sequencer1.client.ledger_get_head_l2_block_height().await;
        assert!(height1.is_ok());

        Ok(())
    }
}

#[tokio::test]
async fn test_multi_sequencer() -> Result<()> {
    TestCaseRunner::new(MutiSequencerTest).run().await
}
