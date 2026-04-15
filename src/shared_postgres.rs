use std::{
    fs::OpenOptions,
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};
use fs2::FileExt;
use tokio::process::Command;

pub const SHARED_POSTGRES_CONTAINER_NAME: &str = "citrea-e2e-shared-postgres";
pub const SHARED_POSTGRES_USER: &str = "clementine";
pub const SHARED_POSTGRES_PASSWORD: &str = "clementine";
pub const SHARED_POSTGRES_IMAGE: &str = "postgres:15";
pub const EXTERNAL_POSTGRES_PORT_ENV: &str = "CITREA_E2E_EXTERNAL_POSTGRES_PORT";

const LOCK_PATH: &str = "/tmp/citrea-e2e-postgres.lock";
const READY_TIMEOUT: Duration = Duration::from_secs(120);

/// Ensure a shared postgres container is running on the host and ready for queries.
///
/// Uses a file lock to serialize concurrent callers across tests/processes.
pub async fn ensure_running(external_port: u16) -> Result<()> {
    let lock_file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(false)
        .open(LOCK_PATH)
        .with_context(|| format!("Failed to open lock file {LOCK_PATH}"))?;

    lock_file
        .lock_exclusive()
        .context("Failed to acquire shared postgres lock")?;

    let result = ensure_running_locked(external_port).await;

    let _ = fs2::FileExt::unlock(&lock_file);
    result
}

async fn ensure_running_locked(external_port: u16) -> Result<()> {
    if !container_exists().await? {
        start_container(external_port).await?;
    }

    wait_for_ready(external_port).await
}

async fn container_exists() -> Result<bool> {
    let output = Command::new("docker")
        .args([
            "ps",
            "--filter",
            &format!("name=^{}$", SHARED_POSTGRES_CONTAINER_NAME),
            "--format",
            "{{.ID}}",
        ])
        .output()
        .await
        .context("Failed to run `docker ps`")?;

    if !output.status.success() {
        bail!(
            "`docker ps` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(!output.stdout.trim_ascii().is_empty())
}

async fn start_container(external_port: u16) -> Result<()> {
    let _ = Command::new("docker")
        .args(["rm", "-f", SHARED_POSTGRES_CONTAINER_NAME])
        .output()
        .await;

    let status = Command::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            SHARED_POSTGRES_CONTAINER_NAME,
            "-p",
            &format!("{external_port}:5432"),
            "-e",
            &format!("POSTGRES_USER={SHARED_POSTGRES_USER}"),
            "-e",
            &format!("POSTGRES_PASSWORD={SHARED_POSTGRES_PASSWORD}"),
            SHARED_POSTGRES_IMAGE,
            "-c",
            "max_connections=1000",
        ])
        .status()
        .await
        .context("Failed to run `docker run` for shared postgres")?;

    if !status.success() {
        bail!("`docker run` for shared postgres failed with status {status}");
    }

    Ok(())
}

async fn wait_for_ready(_external_port: u16) -> Result<()> {
    let start = Instant::now();
    let mut last_err: Option<String> = None;
    while start.elapsed() < READY_TIMEOUT {
        let output = Command::new("docker")
            .args([
                "exec",
                SHARED_POSTGRES_CONTAINER_NAME,
                "pg_isready",
                "-U",
                SHARED_POSTGRES_USER,
                "-d",
                "postgres",
            ])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => return Ok(()),
            Ok(o) => {
                last_err = Some(String::from_utf8_lossy(&o.stderr).to_string());
            }
            Err(e) => {
                last_err = Some(e.to_string());
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    bail!(
        "Shared postgres did not become ready within {READY_TIMEOUT:?}: {}",
        last_err.unwrap_or_default()
    );
}

pub fn external_port_from_env() -> Option<u16> {
    std::env::var(EXTERNAL_POSTGRES_PORT_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
}
