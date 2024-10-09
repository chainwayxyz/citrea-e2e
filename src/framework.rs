use std::{future::Future, sync::Arc};

use bitcoincore_rpc::RpcApi;
use tracing::{debug, info};

use super::{
    bitcoin::BitcoinNodeCluster, config::TestConfig, docker::DockerEnv, full_node::FullNode,
    node::NodeKind, sequencer::Sequencer, traits::NodeT, Result,
};
use crate::{
    batch_prover::BatchProver,
    light_client_prover::LightClientProver,
    log_provider::{LogPathProvider, LogPathProviderErased},
    utils::tail_file,
};

pub struct TestContext {
    pub config: TestConfig,
    pub docker: Arc<Option<DockerEnv>>,
}

impl TestContext {
    async fn new(config: TestConfig) -> Self {
        let docker = if config.test_case.docker {
            Some(DockerEnv::new(config.test_case.n_nodes).await.unwrap())
        } else {
            None
        };
        Self {
            config,
            docker: Arc::new(docker),
        }
    }
}

pub struct TestFramework {
    ctx: TestContext,
    pub bitcoin_nodes: BitcoinNodeCluster,
    pub sequencer: Option<Sequencer>,
    pub batch_prover: Option<BatchProver>,
    pub light_client_prover: Option<LightClientProver>,
    pub full_node: Option<FullNode>,
    pub initial_da_height: u64,
}

async fn create_optional<T>(pred: bool, f: impl Future<Output = Result<T>>) -> Result<Option<T>> {
    if pred {
        Ok(Some(f.await?))
    } else {
        Ok(None)
    }
}

impl TestFramework {
    pub async fn new(config: TestConfig) -> Result<Self> {
        anyhow::ensure!(
            config.test_case.n_nodes > 0,
            "At least one bitcoin node has to be running"
        );

        let ctx = TestContext::new(config).await;

        let bitcoin_nodes = BitcoinNodeCluster::new(&ctx).await?;

        Ok(Self {
            bitcoin_nodes,
            sequencer: None,
            batch_prover: None,
            light_client_prover: None,
            full_node: None,
            ctx,
            initial_da_height: 0,
        })
    }

    pub async fn init_nodes(&mut self) -> Result<()> {
        // Has to initialize sequencer first since provers and full node depend on it
        self.sequencer = create_optional(
            self.ctx.config.test_case.with_sequencer,
            Sequencer::new(&self.ctx.config.sequencer),
        )
        .await?;

        (self.batch_prover, self.light_client_prover, self.full_node) = tokio::try_join!(
            create_optional(
                self.ctx.config.test_case.with_batch_prover,
                BatchProver::new(&self.ctx.config.batch_prover)
            ),
            create_optional(
                self.ctx.config.test_case.with_light_client_prover,
                LightClientProver::new(&self.ctx.config.light_client_prover)
            ),
            create_optional(
                self.ctx.config.test_case.with_full_node,
                FullNode::new(&self.ctx.config.full_node)
            ),
        )?;

        Ok(())
    }

    fn get_nodes_as_log_provider(&self) -> Vec<&dyn LogPathProviderErased> {
        self.ctx
            .config
            .bitcoin
            .iter()
            .map(LogPathProvider::as_erased)
            .chain(vec![
                LogPathProvider::as_erased(&self.ctx.config.sequencer),
                LogPathProvider::as_erased(&self.ctx.config.full_node),
                LogPathProvider::as_erased(&self.ctx.config.batch_prover),
                LogPathProvider::as_erased(&self.ctx.config.light_client_prover),
            ])
            .collect()
    }

    pub fn dump_log(&self) -> Result<()> {
        debug!("Dumping logs:");

        let n_lines = std::env::var("TAIL_N_LINES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(25);
        for provider in self.get_nodes_as_log_provider() {
            println!("{} logs (last {n_lines} lines):", provider.kind());
            let _ = tail_file(&provider.log_path(), n_lines);
            println!("{} stderr logs (last {n_lines} lines):", provider.kind());
            let _ = tail_file(&provider.stderr_path(), n_lines);
        }
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping framework...");

        if let Some(sequencer) = &mut self.sequencer {
            let _ = sequencer.stop().await;
            info!("Successfully stopped sequencer");
        }

        if let Some(batch_prover) = &mut self.batch_prover {
            let _ = batch_prover.stop().await;
            info!("Successfully stopped batch_prover");
        }

        if let Some(light_client_prover) = &mut self.light_client_prover {
            let _ = light_client_prover.stop().await;
            info!("Successfully stopped light_client_prover");
        }

        if let Some(full_node) = &mut self.full_node {
            let _ = full_node.stop().await;
            info!("Successfully stopped full_node");
        }

        let _ = self.bitcoin_nodes.stop_all().await;
        info!("Successfully stopped bitcoin nodes");

        if let Some(docker) = self.ctx.docker.as_ref() {
            let _ = docker.cleanup().await;
            info!("Successfully cleaned docker");
        }

        Ok(())
    }

    pub async fn fund_da_wallets(&mut self) -> Result<()> {
        let da = self.bitcoin_nodes.get(0).unwrap();

        da.create_wallet(&NodeKind::Sequencer.to_string(), None, None, None, None)
            .await?;
        da.create_wallet(&NodeKind::BatchProver.to_string(), None, None, None, None)
            .await?;
        da.create_wallet(
            &NodeKind::LightClientProver.to_string(),
            None,
            None,
            None,
            None,
        )
        .await?;
        da.create_wallet(&NodeKind::Bitcoin.to_string(), None, None, None, None)
            .await?;

        let blocks_to_mature = 100;
        let blocks_to_fund = 25;
        if self.ctx.config.test_case.with_sequencer {
            da.fund_wallet(NodeKind::Sequencer.to_string(), blocks_to_fund)
                .await?;
        }

        if self.ctx.config.test_case.with_batch_prover {
            da.fund_wallet(NodeKind::BatchProver.to_string(), blocks_to_fund)
                .await?;
        }

        if self.ctx.config.test_case.with_light_client_prover {
            da.fund_wallet(NodeKind::LightClientProver.to_string(), blocks_to_fund)
                .await?;
        }

        da.fund_wallet(NodeKind::Bitcoin.to_string(), blocks_to_fund)
            .await?;

        da.generate(blocks_to_mature, None).await?;
        self.initial_da_height = da.get_block_count().await?;
        Ok(())
    }
}
