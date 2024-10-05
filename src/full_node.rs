use anyhow::bail;
use sov_rollup_interface::rpc::{SequencerCommitmentResponse, VerifiedProofResponse};
use tokio::time::{sleep, Duration, Instant};

use super::{config::FullFullNodeConfig, Result};
use crate::node::Node;

pub type FullNode = Node<FullFullNodeConfig>;

impl FullNode {
    pub async fn wait_for_sequencer_commitments(
        &self,
        height: u64,
        timeout: Option<Duration>,
    ) -> Result<Vec<SequencerCommitmentResponse>> {
        let start = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(30));

        loop {
            if start.elapsed() >= timeout {
                bail!("FullNode failed to get sequencer commitments within the specified timeout");
            }

            match self
                .client
                .ledger_get_sequencer_commitments_on_slot_by_number(height)
                .await
            {
                Ok(Some(commitments)) => return Ok(commitments),
                Ok(None) => sleep(Duration::from_millis(500)).await,
                Err(e) => bail!("Error fetching sequencer commitments: {}", e),
            }
        }
    }

    pub async fn wait_for_zkproofs(
        &self,
        height: u64,
        timeout: Option<Duration>,
    ) -> Result<Vec<VerifiedProofResponse>> {
        let start = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(30));

        loop {
            if start.elapsed() >= timeout {
                bail!("FullNode failed to get zkproofs within the specified timeout");
            }

            match self
                .client
                .ledger_get_verified_proofs_by_slot_height(height)
                .await?
            {
                Some(proofs) => return Ok(proofs),
                None => sleep(Duration::from_millis(500)).await,
            }
        }
    }
}
