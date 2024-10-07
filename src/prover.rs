use std::time::SystemTime;

use super::{config::FullProverConfig, Result};
use crate::node::Node;
use anyhow::bail;
use log::debug;
use tokio::time::{sleep, Duration};

pub type Prover = Node<FullProverConfig>;

impl Prover {
    pub async fn wait_for_l1_height(&self, height: u64, timeout: Option<Duration>) -> Result<()> {
        let start = SystemTime::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(600));
        loop {
            debug!("Waiting for prover height {}", height);
            let latest_block = self.client.ledger_get_last_scanned_l1_height().await?;

            if latest_block >= height {
                break;
            }

            let now = SystemTime::now();
            if start + timeout <= now {
                bail!("Timeout. Latest prover L1 height is {}", latest_block);
            }

            sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }
}
