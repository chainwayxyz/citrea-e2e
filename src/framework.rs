use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::{Arc, Once},
};

use anyhow::Context;
use bitcoincore_rpc::RpcApi;
use tracing::{debug, info};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use super::{
    bitcoin::BitcoinNodeCluster,
    config::{
        BitcoinConfig, FullBatchProverConfig, FullFullNodeConfig, FullSequencerConfig,
        RollupConfig, TestCaseConfig, TestConfig,
    },
    docker::DockerEnv,
    full_node::FullNode,
    node::NodeKind,
    sequencer::Sequencer,
    traits::NodeT,
    utils::{copy_directory, get_available_port},
    Result,
};
use crate::{
    batch_prover::BatchProver,
    config::{
        BitcoinServiceConfig, FullLightClientProverConfig, RpcConfig, RunnerConfig, StorageConfig,
    },
    light_client_prover::LightClientProver,
    log_provider::{LogPathProvider, LogPathProviderErased},
    test_case::TestCase,
    utils::{get_default_genesis_path, get_workspace_root, tail_file},
};

pub struct TestContext {
    pub config: TestConfig,
    pub docker: Arc<Option<DockerEnv>>,
}

impl TestContext {
    fn new(config: TestConfig, docker: Option<DockerEnv>) -> Self {
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
    pub async fn new<T: TestCase>() -> Result<Self> {
        setup_logging();

        let test_case = T::test_config();
        let docker = if test_case.docker.enabled() {
            Some(DockerEnv::new(test_case.docker.clone()).await?)
        } else {
            None
        };
        let config = generate_test_config::<T>(test_case, &docker)?;

        anyhow::ensure!(
            config.test_case.n_nodes > 0,
            "At least one bitcoin node has to be running"
        );

        let ctx = TestContext::new(config, docker);

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
        // Use first node config for now, as citrea nodes are expected to interact only with this main node for now.
        // Additional bitcoin node are solely used for simulating a bitcoin network and tx propagation/re-orgs
        let bitcoin_config = &self.ctx.config.bitcoin[0];

        // Has to initialize sequencer first since provers and full node depend on it
        self.sequencer = create_optional(
            self.ctx.config.test_case.with_sequencer,
            Sequencer::new(
                &self.ctx.config.sequencer,
                bitcoin_config,
                Arc::clone(&self.ctx.docker),
            ),
        )
        .await?;

        (self.batch_prover, self.light_client_prover, self.full_node) = tokio::try_join!(
            create_optional(
                self.ctx.config.test_case.with_batch_prover,
                BatchProver::new(
                    &self.ctx.config.batch_prover,
                    bitcoin_config,
                    Arc::clone(&self.ctx.docker)
                )
            ),
            create_optional(
                self.ctx.config.test_case.with_light_client_prover,
                LightClientProver::new(
                    &self.ctx.config.light_client_prover,
                    bitcoin_config,
                    Arc::clone(&self.ctx.docker)
                )
            ),
            create_optional(
                self.ctx.config.test_case.with_full_node,
                FullNode::new(
                    &self.ctx.config.full_node,
                    bitcoin_config,
                    Arc::clone(&self.ctx.docker)
                )
            ),
        )?;

        Ok(())
    }

    fn get_nodes_as_log_provider(&self) -> Vec<&dyn LogPathProviderErased> {
        let test_case = &self.ctx.config.test_case;

        self.ctx
            .config
            .bitcoin
            .iter()
            .map(LogPathProvider::as_erased)
            .map(Option::Some)
            .chain(vec![
                test_case
                    .with_sequencer
                    .then(|| LogPathProvider::as_erased(&self.ctx.config.sequencer)),
                test_case
                    .with_full_node
                    .then(|| LogPathProvider::as_erased(&self.ctx.config.full_node)),
                test_case
                    .with_batch_prover
                    .then(|| LogPathProvider::as_erased(&self.ctx.config.batch_prover)),
                test_case
                    .with_light_client_prover
                    .then(|| LogPathProvider::as_erased(&self.ctx.config.light_client_prover)),
            ])
            .flatten()
            .collect()
    }

    pub fn dump_logs(&self) -> Result<()> {
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
        for da in self.bitcoin_nodes.iter() {
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
        }

        let da = self.bitcoin_nodes.get(0).unwrap();

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

        da.generate(blocks_to_mature).await?;
        self.initial_da_height = da.get_block_count().await?;
        Ok(())
    }
}

fn generate_test_config<T: TestCase>(
    test_case: TestCaseConfig,
    docker: &Option<DockerEnv>,
) -> Result<TestConfig> {
    let env = T::test_env();
    let bitcoin = T::bitcoin_config();
    let batch_prover = T::batch_prover_config();
    let light_client_prover = T::light_client_prover_config();
    let sequencer = T::sequencer_config();
    let sequencer_rollup = RollupConfig::default();
    let batch_prover_rollup = RollupConfig::default();
    let light_client_prover_rollup = RollupConfig::default();
    let full_node_rollup = RollupConfig::default();

    let [bitcoin_dir, dbs_dir, batch_prover_dir, light_client_prover_dir, sequencer_dir, full_node_dir, genesis_dir, tx_backup_dir] =
        create_dirs(&test_case.dir)?;

    copy_genesis_dir(&test_case.genesis_dir, &genesis_dir)?;

    let mut bitcoin_confs = vec![];
    for i in 0..test_case.n_nodes {
        let data_dir = bitcoin_dir.join(i.to_string());
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create {} directory", data_dir.display()))?;

        let p2p_port = get_available_port()?;
        let rpc_port = get_available_port()?;

        bitcoin_confs.push(BitcoinConfig {
            p2p_port,
            rpc_port,
            data_dir,
            env: env.bitcoin().clone(),
            idx: i,
            ..bitcoin.clone()
        });
    }

    bitcoin_confs[0].docker_host = docker
        .as_ref()
        .and_then(|d| d.citrea().then(|| d.get_hostname(&NodeKind::Bitcoin)));

    // Target first bitcoin node as DA for now
    let da_config: BitcoinServiceConfig = bitcoin_confs[0].clone().into();

    let runner_bind_host = match docker.as_ref() {
        Some(d) if d.citrea() => d.get_hostname(&NodeKind::Sequencer),
        _ => sequencer_rollup.rpc.bind_host.clone(),
    };

    let bind_host = match docker.as_ref() {
        Some(d) if d.citrea() => "0.0.0.0".to_string(),
        _ => sequencer_rollup.rpc.bind_host.clone(),
    };

    let sequencer_rollup = {
        let bind_port = get_available_port()?;
        let node_kind = NodeKind::Sequencer.to_string();
        RollupConfig {
            da: BitcoinServiceConfig {
                da_private_key: Some(
                    "045FFC81A3C1FDB3AF1359DBF2D114B0B3EFBF7F29CC9C5DA01267AA39D2C78D".to_string(),
                ),
                node_url: format!("http://{}/wallet/{}", da_config.node_url, node_kind),
                tx_backup_dir: tx_backup_dir.display().to_string(),
                ..da_config.clone()
            },
            storage: StorageConfig {
                path: dbs_dir.join(format!("{node_kind}-db")),
                db_max_open_files: None,
            },
            rpc: RpcConfig {
                bind_port,
                bind_host: bind_host.clone(),
                ..sequencer_rollup.rpc
            },
            ..sequencer_rollup
        }
    };

    let runner_config = Some(RunnerConfig {
        sequencer_client_url: format!(
            "http://{}:{}",
            runner_bind_host, sequencer_rollup.rpc.bind_port,
        ),
        include_tx_body: true,
        sync_blocks_count: 10,
        pruning_config: None,
    });

    let batch_prover_rollup = {
        let bind_port = get_available_port()?;
        let node_kind = NodeKind::BatchProver.to_string();
        RollupConfig {
            da: BitcoinServiceConfig {
                da_private_key: Some(
                    "75BAF964D074594600366E5B111A1DA8F86B2EFE2D22DA51C8D82126A0FCAC72".to_string(),
                ),
                node_url: format!("http://{}/wallet/{}", da_config.node_url, node_kind),
                tx_backup_dir: tx_backup_dir.display().to_string(),
                ..da_config.clone()
            },
            storage: StorageConfig {
                path: dbs_dir.join(format!("{node_kind}-db")),
                db_max_open_files: None,
            },
            rpc: RpcConfig {
                bind_port,
                bind_host: bind_host.clone(),
                ..batch_prover_rollup.rpc
            },
            runner: runner_config.clone(),
            ..batch_prover_rollup
        }
    };

    let light_client_prover_rollup = {
        let bind_port = get_available_port()?;
        let node_kind = NodeKind::LightClientProver.to_string();
        RollupConfig {
            da: BitcoinServiceConfig {
                da_private_key: None,
                node_url: format!("http://{}/wallet/{}", da_config.node_url, node_kind),
                tx_backup_dir: tx_backup_dir.display().to_string(),
                ..da_config.clone()
            },
            storage: StorageConfig {
                path: dbs_dir.join(format!("{node_kind}-db")),
                db_max_open_files: None,
            },
            rpc: RpcConfig {
                bind_port,
                bind_host: bind_host.clone(),
                ..light_client_prover_rollup.rpc
            },
            runner: runner_config.clone(),
            ..light_client_prover_rollup
        }
    };

    let full_node_rollup = {
        let bind_port = get_available_port()?;
        let node_kind = NodeKind::FullNode.to_string();
        RollupConfig {
            da: BitcoinServiceConfig {
                node_url: format!(
                    "http://{}/wallet/{}",
                    da_config.node_url,
                    NodeKind::Bitcoin // Use default wallet
                ),
                tx_backup_dir: tx_backup_dir.display().to_string(),
                ..da_config.clone()
            },
            storage: StorageConfig {
                path: dbs_dir.join(format!("{node_kind}-db")),
                db_max_open_files: None,
            },
            rpc: RpcConfig {
                bind_port,
                bind_host: bind_host.clone(),
                ..full_node_rollup.rpc
            },
            runner: runner_config.clone(),
            ..full_node_rollup
        }
    };

    Ok(TestConfig {
        bitcoin: bitcoin_confs,
        sequencer: FullSequencerConfig::new(
            sequencer,
            sequencer_rollup,
            None,
            sequencer_dir,
            env.sequencer(),
        )?,
        batch_prover: FullBatchProverConfig::new(
            batch_prover,
            batch_prover_rollup,
            None,
            batch_prover_dir,
            env.batch_prover(),
        )?,
        light_client_prover: FullLightClientProverConfig::new(
            light_client_prover,
            light_client_prover_rollup,
            None,
            light_client_prover_dir,
            env.light_client_prover(),
        )?,
        full_node: FullFullNodeConfig::new(
            (),
            full_node_rollup,
            None,
            full_node_dir,
            env.full_node(),
        )?,
        test_case,
    })
}

fn create_dirs(base_dir: &Path) -> Result<[PathBuf; 8]> {
    let paths = [
        NodeKind::Bitcoin.to_string(),
        "dbs".to_string(),
        NodeKind::BatchProver.to_string(),
        NodeKind::LightClientProver.to_string(),
        NodeKind::Sequencer.to_string(),
        NodeKind::FullNode.to_string(),
        "genesis".to_string(),
        "inscription_txs".to_string(),
    ]
    .map(|dir| base_dir.join(dir));

    for path in &paths {
        std::fs::create_dir_all(path)
            .with_context(|| format!("Failed to create {} directory", path.display()))?;
    }

    Ok(paths)
}

fn copy_genesis_dir(genesis_dir: &Option<String>, target_dir: &Path) -> std::io::Result<()> {
    let genesis_dir =
        genesis_dir
            .as_ref()
            .map(PathBuf::from)
            .map_or_else(get_default_genesis_path, |dir| {
                if dir.is_absolute() {
                    dir
                } else {
                    get_workspace_root().join(dir)
                }
            });

    copy_directory(genesis_dir, target_dir)
}

static INIT: Once = Once::new();

fn setup_logging() {
    INIT.call_once(|| {
        let env_filter = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new("citrea_e2e=info"))
            .unwrap();

        if std::env::var("JSON_LOGS").is_ok() {
            let _ = tracing_subscriber::registry()
                .with(fmt::layer().json())
                .with(env_filter)
                .try_init();
        } else {
            let _ = tracing_subscriber::registry()
                .with(fmt::layer())
                .with(env_filter)
                .try_init();
        }
    });
}
