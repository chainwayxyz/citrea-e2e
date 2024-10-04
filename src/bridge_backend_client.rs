use std::{net::SocketAddr, time::Duration};

use jsonrpsee::{
    http_client::{HttpClient, HttpClientBuilder},
    ws_client::{PingConfig, WsClient, WsClientBuilder},
};

pub const MAX_FEE_PER_GAS: u128 = 1000000001;

pub struct BridgeBackendClient {
    http_client: HttpClient,
    ws_client: WsClient,
    pub rpc_addr: SocketAddr,
}

impl BridgeBackendClient {
    pub async fn new(rpc_addr: SocketAddr) -> anyhow::Result<Self> {
        let http_host = format!("http://localhost:{}", rpc_addr.port());
        let ws_host = format!("ws://localhost:{}", rpc_addr.port());

        let http_client = HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(120))
            .build(http_host)?;

        let ws_client = WsClientBuilder::default()
            .enable_ws_ping(PingConfig::default().inactive_limit(Duration::from_secs(10)))
            .build(ws_host)
            .await?;

        Ok(Self {
            ws_client,
            http_client,
            rpc_addr,
        })
    }
}
