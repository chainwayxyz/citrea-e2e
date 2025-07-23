use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tonic::{
    transport::{Certificate, Channel, ClientTlsConfig, Identity},
    Request,
};

pub mod clementine {
    tonic::include_proto!("clementine");
}

use clementine::{
    clementine_aggregator_client::ClementineAggregatorClient,
    clementine_operator_client::ClementineOperatorClient,
    clementine_verifier_client::ClementineVerifierClient, *,
};

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub client_cert_path: PathBuf,
    pub client_key_path: PathBuf,
    pub ca_cert_path: PathBuf,
    pub server_name: String,
}

impl TlsConfig {
    pub fn new(
        client_cert_path: impl AsRef<Path>,
        client_key_path: impl AsRef<Path>,
        ca_cert_path: impl AsRef<Path>,
        server_name: String,
    ) -> Self {
        Self {
            client_cert_path: client_cert_path.as_ref().to_path_buf(),
            client_key_path: client_key_path.as_ref().to_path_buf(),
            ca_cert_path: ca_cert_path.as_ref().to_path_buf(),
            server_name,
        }
    }

    pub async fn create_channel(&self, endpoint: String) -> Result<Channel> {
        let client_cert = tokio::fs::read(&self.client_cert_path)
            .await
            .with_context(|| format!("Failed to read client cert: {:?}", self.client_cert_path))?;

        let client_key = tokio::fs::read(&self.client_key_path)
            .await
            .with_context(|| format!("Failed to read client key: {:?}", self.client_key_path))?;

        let ca_cert = tokio::fs::read(&self.ca_cert_path)
            .await
            .with_context(|| format!("Failed to read CA cert: {:?}", self.ca_cert_path))?;

        let identity = Identity::from_pem(&client_cert, &client_key);
        let ca = Certificate::from_pem(&ca_cert);

        let tls_config = ClientTlsConfig::new()
            .ca_certificate(ca)
            .identity(identity)
            .domain_name(&self.server_name);

        let channel = Channel::from_shared(endpoint)?
            .tls_config(tls_config)?
            .connect()
            .await
            .context("Failed to connect to gRPC server")?;

        Ok(channel)
    }
}

#[derive(Debug, Clone)]
pub struct ClementineAggregatorTestClient {
    client: ClementineAggregatorClient<Channel>,
}

impl ClementineAggregatorTestClient {
    pub async fn new(endpoint: String, tls_config: TlsConfig) -> Result<Self> {
        let channel = tls_config.create_channel(endpoint).await?;
        let client = ClementineAggregatorClient::new(channel);
        Ok(Self { client })
    }

    pub async fn new_deposit(&mut self, deposit: Deposit) -> Result<RawSignedTx> {
        let request = Request::new(deposit);
        let response = self
            .client
            .new_deposit(request)
            .await
            .context("Failed to call new_deposit")?;
        Ok(response.into_inner())
    }

    pub async fn withdraw(&mut self, params: WithdrawParams) -> Result<AggregatorWithdrawResponse> {
        let request = Request::new(params);
        let response = self
            .client
            .withdraw(request)
            .await
            .context("Failed to call withdraw")?;
        Ok(response.into_inner())
    }

    pub async fn optimistic_payout(&mut self, params: WithdrawParams) -> Result<RawSignedTx> {
        let request = Request::new(params);
        let response = self
            .client
            .optimistic_payout(request)
            .await
            .context("Failed to call optimistic_payout")?;
        Ok(response.into_inner())
    }

    pub async fn setup(&mut self) -> Result<VerifierPublicKeys> {
        let request = Request::new(Empty {});
        let response = self
            .client
            .setup(request)
            .await
            .context("Failed to call setup")?;
        Ok(response.into_inner())
    }

    pub async fn get_nofn_aggregated_xonly_pk(&mut self) -> Result<NofnResponse> {
        let request = Request::new(Empty {});
        let response = self
            .client
            .get_nofn_aggregated_xonly_pk(request)
            .await
            .context("Failed to call get_nofn_aggregated_xonly_pk")?;
        Ok(response.into_inner())
    }

    pub async fn get_entity_statuses(&mut self, restart_tasks: bool) -> Result<EntityStatuses> {
        let request = Request::new(GetEntityStatusesRequest { restart_tasks });
        let response = self
            .client
            .get_entity_statuses(request)
            .await
            .context("Failed to call get_entity_statuses")?;
        Ok(response.into_inner())
    }
}

#[derive(Debug, Clone)]
pub struct ClementineOperatorTestClient {
    client: ClementineOperatorClient<Channel>,
}

impl ClementineOperatorTestClient {
    pub async fn new(endpoint: String, tls_config: TlsConfig) -> Result<Self> {
        let channel = tls_config.create_channel(endpoint).await?;
        let client = ClementineOperatorClient::new(channel);
        Ok(Self { client })
    }

    pub async fn get_x_only_public_key(&mut self) -> Result<XOnlyPublicKeyRpc> {
        let request = Request::new(Empty {});
        let response = self
            .client
            .get_x_only_public_key(request)
            .await
            .context("Failed to call get_x_only_public_key")?;
        Ok(response.into_inner())
    }

    pub async fn get_current_status(&mut self) -> Result<EntityStatus> {
        let request = Request::new(Empty {});
        let response = self
            .client
            .get_current_status(request)
            .await
            .context("Failed to call get_current_status")?;
        Ok(response.into_inner())
    }

    pub async fn get_deposit_keys(&mut self, params: DepositParams) -> Result<OperatorKeys> {
        let request = Request::new(params);
        let response = self
            .client
            .get_deposit_keys(request)
            .await
            .context("Failed to call get_deposit_keys")?;
        Ok(response.into_inner())
    }

    pub async fn withdraw(&mut self, params: WithdrawParams) -> Result<()> {
        let request = Request::new(params);
        self.client
            .withdraw(request)
            .await
            .context("Failed to call withdraw")?;
        Ok(())
    }

    pub async fn restart_background_tasks(&mut self) -> Result<()> {
        let request = Request::new(Empty {});
        self.client
            .restart_background_tasks(request)
            .await
            .context("Failed to call restart_background_tasks")?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ClementineVerifierTestClient {
    client: ClementineVerifierClient<Channel>,
}

impl ClementineVerifierTestClient {
    pub async fn new(endpoint: String, tls_config: TlsConfig) -> Result<Self> {
        let channel = tls_config.create_channel(endpoint).await?;
        let client = ClementineVerifierClient::new(channel);
        Ok(Self { client })
    }

    pub async fn get_params(&mut self) -> Result<VerifierParams> {
        let request = Request::new(Empty {});
        let response = self
            .client
            .get_params(request)
            .await
            .context("Failed to call get_params")?;
        Ok(response.into_inner())
    }

    pub async fn get_current_status(&mut self) -> Result<EntityStatus> {
        let request = Request::new(Empty {});
        let response = self
            .client
            .get_current_status(request)
            .await
            .context("Failed to call get_current_status")?;
        Ok(response.into_inner())
    }

    pub async fn set_operator_keys(&mut self, keys: OperatorKeysWithDeposit) -> Result<()> {
        let request = Request::new(keys);
        self.client
            .set_operator_keys(request)
            .await
            .context("Failed to call set_operator_keys")?;
        Ok(())
    }

    pub async fn restart_background_tasks(&mut self) -> Result<()> {
        let request = Request::new(Empty {});
        self.client
            .restart_background_tasks(request)
            .await
            .context("Failed to call restart_background_tasks")?;
        Ok(())
    }

    pub async fn debug_tx(&mut self, tx_id: u32) -> Result<TxDebugInfo> {
        let request = Request::new(TxDebugRequest { tx_id });
        let response = self
            .client
            .debug_tx(request)
            .await
            .context("Failed to call debug_tx")?;
        Ok(response.into_inner())
    }
}
