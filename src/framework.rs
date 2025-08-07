use std::{
    path::{Path, PathBuf},
    sync::{Arc, Once},
};

use anyhow::Context;
use bitcoincore_rpc::RpcApi;
use tracing::{debug, info};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[cfg(feature = "clementine")]
use crate::clementine::ClementineCluster;
// Conditional imports for clementine features
use crate::{
    bitcoin::BitcoinNodeCluster,
    citrea_cli::CitreaCli,
    config::{
        BitcoinConfig, BitcoinServiceConfig, EmptyConfig, FullBatchProverConfig,
        FullFullNodeConfig, FullLightClientProverConfig, FullSequencerConfig, ListenModeConfig,
        RollupConfig, RpcConfig, RunnerConfig, StorageConfig, TestCaseConfig, TestConfig,
    },
    docker::DockerEnv,
    log_provider::{LogPathProvider, LogPathProviderErased},
    node::{BatchProver, FullNode, LightClientProver, NodeKind, Sequencer},
    sequencer::SequencerCluster,
    test_case::{TestCase, CITREA_CLI_ENV},
    traits::NodeT,
    utils::{
        copy_directory, create_optional, get_available_port, get_default_genesis_path,
        get_workspace_root, tail_file,
    },
    Result,
};
use crate::{clementine::ClementineIntegration, config::SequencerConfig};
#[cfg(feature = "clementine")]
use crate::{config::PostgresConfig, postgres::Postgres};

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
    #[cfg(feature = "clementine")]
    pub postgres: Option<Postgres>,
    pub sequencer: Option<Sequencer>,
    pub sequencer_cluster: Option<SequencerCluster>,
    pub batch_prover: Option<BatchProver>,
    pub light_client_prover: Option<LightClientProver>,
    pub full_node: Option<FullNode>,
    #[cfg(feature = "clementine")]
    pub clementine_nodes: Option<ClementineCluster>,
    pub initial_da_height: u64,
    pub citrea_cli: Option<CitreaCli>,
}

impl TestFramework {
    pub async fn new<T: TestCase>() -> Result<Self> {
        setup_logging();
        ClementineIntegration::init_certificates().await?;

        let test_case = T::test_config();
        let docker = if test_case.docker_enabled() {
            Some(DockerEnv::new(test_case.docker.clone()).await?)
        } else {
            None
        };
        let citrea_cli = match &test_case.with_citrea_cli {
            false => None,
            true => Some(CitreaCli::new(CITREA_CLI_ENV)?),
        };
        let config = generate_test_config::<T>(test_case, &docker)?;

        anyhow::ensure!(
            config.test_case.get_n_nodes(NodeKind::Bitcoin) > 0,
            "At least one bitcoin node has to be running"
        );

        let ctx = TestContext::new(config, docker);

        let bitcoin_nodes = BitcoinNodeCluster::new(&ctx).await?;

        #[cfg(feature = "clementine")]
        let postgres = if ctx.config.test_case.with_clementine {
            Some(Postgres::new(&ctx.config.postgres, Arc::clone(&ctx.docker)).await?)
        } else {
            None
        };

        Ok(Self {
            bitcoin_nodes,
            #[cfg(feature = "clementine")]
            postgres,
            sequencer: None,
            sequencer_cluster: None,
            batch_prover: None,
            light_client_prover: None,
            full_node: None,
            #[cfg(feature = "clementine")]
            clementine_nodes: None,
            ctx,
            initial_da_height: 0,
            citrea_cli,
        })
    }

    #[cfg(feature = "clementine")]
    pub async fn init_clementine_nodes(&mut self) -> Result<()> {
        self.clementine_nodes = ClementineIntegration::init_nodes(
            &self.ctx.config.clementine,
            Arc::clone(&self.ctx.docker),
            self.ctx.config.test_case.with_clementine,
        )
        .await?;

        Ok(())
    }

    pub async fn init_citrea_nodes(&mut self) -> Result<()> {
        // Use first node config for now, as citrea nodes are expected to interact only with this main node for now.
        // Additional bitcoin node are solely used for simulating a bitcoin network and tx propagation/re-orgs
        let bitcoin_config = &self.ctx.config.bitcoin[0];

        // Has to initialize sequencer first since provers and full node depend on it
        if self.ctx.config.test_case.get_n_nodes(NodeKind::Sequencer) > 1 {
            self.sequencer_cluster = create_optional(
                self.ctx.config.test_case.with_sequencer,
                SequencerCluster::new(&self.ctx),
            )
            .await?;
        } else {
            self.sequencer = create_optional(
                self.ctx.config.test_case.with_sequencer,
                Sequencer::new(
                    &self.ctx.config.sequencer[0],
                    bitcoin_config,
                    Arc::clone(&self.ctx.docker),
                ),
            )
            .await?;
        }

        // Initialize nodes individually with clear conditional logic
        self.batch_prover = create_optional(
            self.ctx.config.test_case.with_batch_prover,
            BatchProver::new(
                &self.ctx.config.batch_prover,
                bitcoin_config,
                Arc::clone(&self.ctx.docker),
            ),
        )
        .await?;

        self.light_client_prover = create_optional(
            self.ctx.config.test_case.with_light_client_prover,
            LightClientProver::new(
                &self.ctx.config.light_client_prover,
                bitcoin_config,
                Arc::clone(&self.ctx.docker),
            ),
        )
        .await?;

        self.full_node = create_optional(
            self.ctx.config.test_case.with_full_node,
            FullNode::new(
                &self.ctx.config.full_node,
                bitcoin_config,
                Arc::clone(&self.ctx.docker),
            ),
        )
        .await?;

        Ok(())
    }

    pub async fn init_nodes(&mut self) -> Result<()> {
        self.init_citrea_nodes().await?;
        #[cfg(feature = "clementine")]
        self.init_clementine_nodes().await?;
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
                // TODO handle all nodes
                test_case
                    .with_sequencer
                    .then(|| LogPathProvider::as_erased(&self.ctx.config.sequencer[0])),
                test_case
                    .with_full_node
                    .then(|| LogPathProvider::as_erased(&self.ctx.config.full_node)),
                test_case
                    .with_batch_prover
                    .then(|| LogPathProvider::as_erased(&self.ctx.config.batch_prover)),
                test_case
                    .with_light_client_prover
                    .then(|| LogPathProvider::as_erased(&self.ctx.config.light_client_prover)),
                #[cfg(feature = "clementine")]
                test_case
                    .with_clementine
                    .then(|| LogPathProvider::as_erased(&self.ctx.config.clementine.aggregator)),
            ])
            .chain({
                #[cfg_attr(not(feature = "clementine"), allow(unused_mut))]
                let mut clementine_providers = Vec::new();
                #[cfg(feature = "clementine")]
                if test_case.with_clementine {
                    clementine_providers.extend(
                        self.ctx
                            .config
                            .clementine
                            .operators
                            .iter()
                            .map(LogPathProvider::as_erased)
                            .chain(
                                self.ctx
                                    .config
                                    .clementine
                                    .verifiers
                                    .iter()
                                    .map(LogPathProvider::as_erased),
                            )
                            .map(Option::Some),
                    );
                }
                clementine_providers
            })
            .flatten()
            .collect()
    }

    pub fn dump_logs(&self) -> Result<()> {
        debug!("Dumping logs:");

        let forced_dump = std::env::var("ENABLE_DUMP_LOGS")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_lowercase())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let n_lines = std::env::var("TAIL_N_LINES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(50);

        for provider in self.get_nodes_as_log_provider() {
            let kind = provider.kind().to_string().to_lowercase();
            let force_dump =
                forced_dump.contains(&kind) || forced_dump.contains(&"all".to_string());

            let had_error = has_errors_or_panics(&provider.log_path())?;
            if force_dump || had_error {
                println!("{} logs (last {n_lines} lines):", provider.kind());

                let _ = tail_file(
                    &provider.log_path(),
                    if had_error { n_lines * 10 } else { n_lines },
                );
            }

            let had_error = has_errors_or_panics(&provider.stderr_path())?;
            if force_dump || had_error {
                println!("{} stderr logs (last {n_lines} lines):", provider.kind());
                let _ = tail_file(
                    &provider.stderr_path(),
                    if had_error { n_lines * 10 } else { n_lines },
                );
            }
        }
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping framework...");

        #[cfg(feature = "clementine")]
        {
            let _ = ClementineIntegration::stop_nodes(&mut self.clementine_nodes).await;
        }

        if let Some(sequencer) = &mut self.sequencer {
            let _ = sequencer.stop().await;
            info!("Successfully stopped sequencer");
        }

        if let Some(cluster) = &mut self.sequencer_cluster {
            let _ = cluster.stop_all().await;
            info!("Successfully stopped sequencer cluster");
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

        if let Some(docker) = self.ctx.docker.as_ref() {
            let _ = docker.cleanup().await;
            info!("Successfully cleaned docker");
        }

        #[cfg(feature = "clementine")]
        if let Some(postgres) = &mut self.postgres {
            let _ = postgres.stop().await;
            info!("Successfully stopped postgres");
        }

        // Stop bitcoin last
        let _ = self.bitcoin_nodes.stop_all().await;
        info!("Successfully stopped bitcoin nodes");

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
    let mut light_client_prover = T::light_client_prover_config();
    let sequencer_rollup = RollupConfig::default();
    let batch_prover_rollup = RollupConfig::default();
    let light_client_prover_rollup = RollupConfig::default();
    let full_node_rollup = RollupConfig::default();
    let scan_l1_start_height = T::scan_l1_start_height();
    light_client_prover.initial_da_height = scan_l1_start_height.unwrap_or(120);
    let throttle_config = T::throttle_config();

    let [bitcoin_dir, dbs_dir, batch_prover_dir, light_client_prover_dir, sequencer_dir, full_node_dir, genesis_dir, tx_backup_dir, _postgres_dir, clementine_dir] =
        create_dirs(&test_case.dir)?;

    copy_genesis_dir(&test_case.genesis_dir, &genesis_dir)?;
    ClementineIntegration::copy_resources(&test_case.clementine_dir, &clementine_dir)?;

    let mut bitcoin_confs = vec![];
    for i in 0..test_case.get_n_nodes(NodeKind::Bitcoin) {
        let data_dir = bitcoin_dir.join(i.to_string());
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create {} directory", data_dir.display()))?;

        let rpc_port = get_available_port()?;
        let p2p_port = get_available_port()?;

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

    let citrea_bind_host = match docker.as_ref() {
        Some(d) if d.citrea() => "0.0.0.0".to_string(),
        _ => sequencer_rollup.rpc.bind_host.clone(),
    };

    let sequencer_configs = generate_sequencer_configs::<T>(
        &test_case,
        da_config.clone(),
        &sequencer_dir,
        &tx_backup_dir,
        &dbs_dir,
        &citrea_bind_host,
    )?;

    let runner_config = Some(RunnerConfig {
        sequencer_client_url: format!(
            "http://{}:{}",
            runner_bind_host, sequencer_configs[0].rollup.rpc.bind_port,
        ),
        include_tx_body: true,
        sync_blocks_count: 10,
        pruning_config: None,
        scan_l1_start_height,
    });

    let batch_prover_rollup = {
        let bind_port = get_available_port()?;
        let node_kind = NodeKind::BatchProver.to_string();
        RollupConfig {
            da: BitcoinServiceConfig {
                da_private_key: Some(
                    "56D08C2DDE7F412F80EC99A0A328F76688C904BD4D1435281EFC9270EC8C8707".to_string(),
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
                bind_host: citrea_bind_host.clone(),
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
                bind_host: citrea_bind_host.clone(),
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
                bind_host: citrea_bind_host.clone(),
                ..full_node_rollup.rpc
            },
            runner: runner_config.clone(),
            ..full_node_rollup
        }
    };

    #[cfg(feature = "clementine")]
    let (clementine, postgres) = {
        let postgres = PostgresConfig {
            port: get_available_port()?,
            log_dir: _postgres_dir,
            docker_host: docker
                .as_ref()
                .and_then(|d| d.clementine().then(|| d.get_hostname(&NodeKind::Postgres))),
            ..Default::default()
        };

        let mut clementine_btc_conf = bitcoin_confs[0].clone();
        clementine_btc_conf.docker_host = docker
            .as_ref()
            .and_then(|d| d.clementine().then(|| d.get_hostname(&NodeKind::Bitcoin)));

        let clementine = ClementineIntegration::generate_cluster_config::<T>(
            &test_case,
            &clementine_dir,
            postgres.clone(),
            clementine_btc_conf,
            full_node_rollup.rpc.clone(),
            light_client_prover_rollup.rpc.clone(),
        )?;
        (clementine, postgres)
    };

    let citrea_docker_image = std::env::var("CITREA_DOCKER_IMAGE").ok();
    Ok(TestConfig {
        bitcoin: bitcoin_confs,
        sequencer: sequencer_configs,
        batch_prover: FullBatchProverConfig::new(
            NodeKind::BatchProver,
            batch_prover,
            batch_prover_rollup,
            citrea_docker_image.clone(),
            batch_prover_dir,
            env.batch_prover(),
            test_case.mode,
            throttle_config.clone(),
        )?,
        light_client_prover: FullLightClientProverConfig::new(
            NodeKind::LightClientProver,
            light_client_prover,
            light_client_prover_rollup,
            citrea_docker_image.clone(),
            light_client_prover_dir,
            env.light_client_prover(),
            test_case.mode,
            throttle_config.clone(),
        )?,
        full_node: FullFullNodeConfig::new(
            NodeKind::FullNode,
            EmptyConfig,
            full_node_rollup,
            citrea_docker_image,
            full_node_dir,
            env.full_node(),
            test_case.mode,
            throttle_config.clone(),
        )?,
        test_case,
        #[cfg(feature = "clementine")]
        clementine,
        #[cfg(feature = "clementine")]
        postgres,
    })
}

fn generate_sequencer_configs<T: TestCase>(
    test_case: &TestCaseConfig,
    da_config: BitcoinServiceConfig,
    dir: &Path,
    tx_backup_dir: &Path,
    dbs_dir: &Path,
    bind_host: &str,
) -> Result<Vec<FullSequencerConfig>> {
    let main_sequencer = T::sequencer_config();
    let env = T::test_env();
    let throttle_config = T::throttle_config();
    let kind = NodeKind::Sequencer;
    let citrea_docker_image = std::env::var("CITREA_DOCKER_IMAGE").ok();

    let mut main_sequencer_client_url = String::new();

    let mut sequencer_configs = vec![];
    for i in 0..test_case.get_n_nodes(kind) {
        let sequencer_dir = dir.join(i.to_string());
        std::fs::create_dir_all(&sequencer_dir)
            .with_context(|| format!("Failed to create {} directory", sequencer_dir.display()))?;

        let bind_port = get_available_port()?;
        let node_kind = kind.to_string();
        let base_rollup = RollupConfig::default();
        let sequencer_rollup = RollupConfig {
            da: BitcoinServiceConfig {
                da_private_key: Some(
                    "E9873D79C6D87DC0FB6A5778633389F4453213303DA61F20BD67FC233AA33262".to_string(),
                ),
                node_url: format!("http://{}/wallet/{}", da_config.node_url, node_kind),
                tx_backup_dir: tx_backup_dir.display().to_string(),
                ..da_config.clone()
            },
            storage: StorageConfig {
                path: dbs_dir.join(format!("{node_kind}-{i}-db")),
                db_max_open_files: None,
            },
            rpc: RpcConfig {
                bind_port,
                bind_host: bind_host.to_string(),
                ..base_rollup.rpc
            },
            ..base_rollup
        };

        let config = if i == 0 {
            main_sequencer_client_url = format!(
                "http://{}:{}",
                sequencer_rollup.rpc.bind_host, sequencer_rollup.rpc.bind_port,
            );
            main_sequencer.clone()
        } else {
            SequencerConfig {
                listen_mode_config: Some(ListenModeConfig {
                    sequencer_client_url: main_sequencer_client_url.clone(),
                    ..Default::default()
                }),
                ..main_sequencer.clone()
            }
        };

        sequencer_configs.push(FullSequencerConfig::new(
            NodeKind::Sequencer,
            config,
            sequencer_rollup,
            citrea_docker_image.clone(),
            sequencer_dir,
            env.sequencer(),
            test_case.mode,
            throttle_config.clone(),
        )?);
    }

    Ok(sequencer_configs)
}

fn create_dirs(base_dir: &Path) -> Result<[PathBuf; 10]> {
    let paths = [
        NodeKind::Bitcoin.to_string(),
        "dbs".to_string(),
        NodeKind::BatchProver.to_string(),
        NodeKind::LightClientProver.to_string(),
        NodeKind::Sequencer.to_string(),
        NodeKind::FullNode.to_string(),
        "genesis".to_string(),
        "inscription_txs".to_string(),
        NodeKind::Postgres.to_string(),
        "clementine".to_string(),
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

fn has_errors_or_panics(path: &Path) -> Result<bool> {
    use std::{
        fs::File,
        io::{BufRead, BufReader},
    };

    const ERROR_KEYWORDS: [&str; 2] = ["error", "panic"];

    if !path.exists() {
        return Ok(false);
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let lowercase = line.to_lowercase();

        if ERROR_KEYWORDS
            .iter()
            .any(|&keyword| lowercase.contains(keyword))
        {
            return Ok(true);
        }
    }

    Ok(false)
}
