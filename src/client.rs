use std::time::{Duration, SystemTime};

use alloy_primitives::U64;
use anyhow::{bail, Result};
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use tokio::time::sleep;
use tracing::trace;

#[derive(Clone, Debug)]
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

    pub fn http_client(&self) -> &HttpClient {
        &self.client
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
            .request("ledger_getLastScannedL1Height", rpc_params![])
            .await
            .map(|v: U64| v.try_into().expect("U64 to u64 must succeed"))?)
    }

    pub async fn ledger_get_head_l2_block_height(&self) -> Result<u64> {
        Ok(self
            .client
            .request("ledger_getHeadL2BlockHeight", rpc_params![])
            .await
            .map(|v: U64| v.try_into().expect("U64 to u64 must succeed"))?)
    }

    pub async fn wait_for_l2_block(&self, num: u64, timeout: Option<Duration>) -> Result<()> {
        let start = SystemTime::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(30)); // Default 30 seconds timeout
        loop {
            trace!("Waiting for l2 block {num}");
            let latest_block = self.ledger_get_head_l2_block_height().await?;

            if latest_block >= num {
                break;
            }

            let now = SystemTime::now();
            if start + timeout <= now {
                bail!("Timeout. Latest L2 block is {latest_block:?}");
            }

            sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }
}
