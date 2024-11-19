use std::time::SystemTime;

use anyhow::bail;
use tokio::time::{sleep, Duration};
use tracing::trace;

use super::{config::FullFullNodeConfig, Result};
use crate::node::Node;

pub type FullNode = Node<FullFullNodeConfig>;

impl FullNode {
    pub async fn wait_for_l1_height(&self, height: u64, timeout: Option<Duration>) -> Result<()> {
        let start = SystemTime::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(600));
        loop {
            trace!("Waiting for batch prover height {}", height);
            let latest_block = self.client.ledger_get_last_scanned_l1_height().await?;

            if latest_block >= height {
                break;
            }

            let now = SystemTime::now();
            if start + timeout <= now {
                bail!("Timeout. Latest batch prover L1 height is {}", latest_block);
            }

            sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }
}
