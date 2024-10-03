use std::{fs::File, net::SocketAddr, path::PathBuf, process::Stdio, time::SystemTime};

use anyhow::{bail, Context};
use async_trait::async_trait;
use log::debug;
use tokio::{
    process::Command,
    time::{sleep, Duration, Instant},
};

use super::{
    config::{config_to_file, FullProverConfig, TestConfig},
    framework::TestContext,
    node::NodeKind,
    traits::{LogProvider, Node, SpawnOutput},
    utils::{get_citrea_path, get_stderr_path, get_stdout_path, retry},
    Result,
};
use crate::{
    client::{make_test_client, L2Client},
    utils::get_genesis_path,
};

#[allow(unused)]
pub struct Prover {
    spawn_output: SpawnOutput,
    config: FullProverConfig,
    pub client: Box<L2Client>,
}

impl Prover {
    pub async fn new(ctx: &TestContext) -> Result<Self> {
        let TestConfig {
            prover: prover_config,
            ..
        } = &ctx.config;

        let spawn_output = Self::spawn(prover_config)?;

        let socket_addr = SocketAddr::new(
            prover_config
                .rollup
                .rpc
                .bind_host
                .parse()
                .context("Failed to parse bind host")?,
            prover_config.rollup.rpc.bind_port,
        );
        let client = retry(|| async { make_test_client(socket_addr).await }, None).await?;

        Ok(Self {
            spawn_output,
            config: prover_config.to_owned(),
            client,
        })
    }

    pub async fn wait_for_l1_height(&self, height: u64, timeout: Option<Duration>) -> Result<()> {
        let start = SystemTime::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(600));
        loop {
            debug!("Waiting for prover height {}", height);
            let latest_block = self.client.ledger_get_last_scanned_l1_height().await;
            if latest_block >= height {
                break;
            }

            let now = SystemTime::now();
            if start + timeout <= now {
                bail!("Timeout. Latest prover L1 height is {}", latest_block);
            }

            sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }
}

#[async_trait]
impl Node for Prover {
    type Config = FullProverConfig;
    type Client = L2Client;

    fn spawn(config: &Self::Config) -> Result<SpawnOutput> {
        let citrea = get_citrea_path();
        let dir = &config.dir;

        let stdout_file =
            File::create(get_stdout_path(dir)).context("Failed to create stdout file")?;
        let stderr_file =
            File::create(get_stderr_path(dir)).context("Failed to create stderr file")?;

        let config_path = dir.join("prover_config.toml");
        config_to_file(&config.node, &config_path)?;

        let rollup_config_path = dir.join("prover_rollup_config.toml");
        config_to_file(&config.rollup, &rollup_config_path)?;

        Command::new(citrea)
            .arg("--da-layer")
            .arg("bitcoin")
            .arg("--rollup-config-path")
            .arg(rollup_config_path)
            .arg("--prover-config-path")
            .arg(config_path)
            .arg("--genesis-paths")
            .arg(get_genesis_path(
                dir.parent().expect("Couldn't get parent dir"),
            ))
            .envs(config.env.clone())
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .kill_on_drop(true)
            .spawn()
            .context("Failed to spawn citrea process")
            .map(SpawnOutput::Child)
    }

    fn spawn_output(&mut self) -> &mut SpawnOutput {
        &mut self.spawn_output
    }

    async fn wait_for_ready(&self, timeout: Option<Duration>) -> Result<()> {
        let start = Instant::now();
        let timeout = timeout.unwrap_or(Duration::from_secs(30));
        while start.elapsed() < timeout {
            if self
                .client
                .ledger_get_head_soft_confirmation()
                .await
                .is_ok()
            {
                return Ok(());
            }
            sleep(Duration::from_millis(500)).await;
        }
        anyhow::bail!("Prover failed to become ready within the specified timeout")
    }

    fn client(&self) -> &Self::Client {
        &self.client
    }

    fn env(&self) -> Vec<(&'static str, &'static str)> {
        self.config.env.clone()
    }

    fn config_mut(&mut self) -> &mut Self::Config {
        &mut self.config
    }
}

impl LogProvider for Prover {
    fn kind(&self) -> NodeKind {
        NodeKind::Prover
    }

    fn log_path(&self) -> PathBuf {
        get_stdout_path(&self.config.dir)
    }
}
