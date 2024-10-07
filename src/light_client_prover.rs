use std::time::SystemTime;

use anyhow::bail;
use log::debug;
use tokio::time::{sleep, Duration};

use super::{config::FullLightClientProverConfig, Result};
use crate::node::Node;

pub type LightClientProver = Node<FullLightClientProverConfig>;

impl LightClientProver {
    // TODO: remove _l at the end
    pub async fn wait_for_l1_height_l(&self, height: u64, timeout: Option<Duration>) -> Result<()> {
        let start = SystemTime::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(600));
        loop {
            debug!("Waiting for light client prover height {}", height);
            let latest_block = self.client.ledger_get_last_scanned_l1_height().await?;

            if latest_block >= height {
                break;
            }

            let now = SystemTime::now();
            if start + timeout <= now {
                bail!(
                    "Timeout. Latest light client prover L1 height is {}",
                    latest_block
                );
            }

            sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }
}
