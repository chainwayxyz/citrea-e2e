use std::time::{Duration, SystemTime};

use anyhow::{bail, Result};
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use log::debug;
use sov_ledger_rpc::client::RpcClient;
use sov_rollup_interface::rpc::{
    SequencerCommitmentResponse, SoftConfirmationResponse, VerifiedProofResponse,
};
use tokio::time::sleep;

pub struct Client {
    client: HttpClient,
}

impl Client {
    pub fn new(host: &str, port: u16) -> Result<Self> {
        let host = format!("http://{host}:{port}");
        let client = HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(120))
            .build(host)?;
        Ok(Self { client })
    }
}

impl Client {
    // TODO Use SequencerRpcClient trait
    pub async fn send_publish_batch_request(&self) -> Result<()> {
        let r = self
            .client
            .request("citrea_testPublishBlock", rpc_params![])
            .await
            .map_err(Into::into);
        sleep(Duration::from_millis(100)).await;
        r
    }

    pub async fn ledger_get_last_scanned_l1_height(&self) -> Result<u64> {
        Ok(self
            .client
            .request("ledger_getLastScannedL1Hieght", rpc_params![])
            .await?)
    }

    pub async fn ledger_get_head_soft_confirmation(
        &self,
    ) -> Result<Option<SoftConfirmationResponse>> {
        Ok(self.client.get_head_soft_confirmation().await?)
    }

    pub async fn ledger_get_verified_proofs_by_slot_height(
        &self,
        height: u64,
    ) -> Result<Option<Vec<VerifiedProofResponse>>> {
        Ok(self
            .client
            .get_verified_proofs_by_slot_height(height)
            .await?)
    }

    pub async fn ledger_get_sequencer_commitments_on_slot_by_number(
        &self,
        height: u64,
    ) -> Result<Option<Vec<SequencerCommitmentResponse>>> {
        Ok(self
            .client
            .get_sequencer_commitments_on_slot_by_number(height)
            .await?)
    }

    pub async fn ledger_get_soft_confirmation_by_number(
        &self,
        num: u64,
    ) -> Result<Option<SoftConfirmationResponse>> {
        Ok(self.client.get_soft_confirmation_by_number(num).await?)
    }

    pub async fn ledger_get_sequencer_commitments_on_slot_by_hash(
        &self,
        hash: [u8; 32],
    ) -> Result<Option<Vec<SequencerCommitmentResponse>>> {
        self.client
            .get_sequencer_commitments_on_slot_by_hash(hash)
            .await
            .map_err(|e| e.into())
    }

    pub async fn ledger_get_head_soft_confirmation_height(&self) -> Result<u64> {
        Ok(self.client.get_head_soft_confirmation_height().await?)
    }

    pub async fn wait_for_l2_block(&self, num: u64, timeout: Option<Duration>) -> Result<()> {
        let start = SystemTime::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(30)); // Default 30 seconds timeout
        loop {
            debug!("Waiting for soft confirmation {}", num);
            let latest_block = self.client.get_head_soft_confirmation_height().await?;

            if latest_block >= num {
                break;
            }

            let now = SystemTime::now();
            if start + timeout <= now {
                bail!("Timeout. Latest L2 block is {:?}", latest_block);
            }

            sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }
}
