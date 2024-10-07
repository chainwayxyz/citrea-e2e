use std::{net::SocketAddr, time::Duration};

use jsonrpsee::{
    http_client::{HttpClient, HttpClientBuilder},
    ws_client::{PingConfig, WsClient, WsClientBuilder},
};

pub struct BridgeBackendClient {
    _http_client: HttpClient,
    _ws_client: WsClient,
    pub rpc_addr: SocketAddr,
}

impl BridgeBackendClient {
    pub async fn new(rpc_addr: SocketAddr) -> anyhow::Result<Self> {
        let http_host = format!("http://localhost:{}", rpc_addr.port());
        let ws_host = format!("ws://localhost:{}", rpc_addr.port());

        let _http_client = HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(120))
            .build(http_host)?;

        let _ws_client = WsClientBuilder::default()
            .enable_ws_ping(PingConfig::default().inactive_limit(Duration::from_secs(10)))
            .build(ws_host)
            .await?;

        Ok(Self {
            _ws_client,
            _http_client,
            rpc_addr,
        })
    }
}
