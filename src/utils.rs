use std::{
    collections::HashSet,
    fs::{self, File},
    future::Future,
    io::{self, BufRead, BufReader},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use anyhow::anyhow;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use tokio::net::TcpStream;
use tracing::debug;

use super::Result;
#[cfg(feature = "clementine")]
use crate::test_case::CLEMENTINE_ENV;

static RESERVED_PORTS: LazyLock<Mutex<HashSet<u16>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

const DEFAULT_PORT_RANGE_START: u16 = 20_000;
const DEFAULT_PORT_RANGE_END: u16 = 30_000;
const PORT_ALLOCATION_ATTEMPTS: usize = 10_000;
const PORT_RANGE_START_ENV: &str = "CITREA_E2E_PORT_RANGE_START";
const PORT_RANGE_END_ENV: &str = "CITREA_E2E_PORT_RANGE_END";

pub fn get_available_port() -> Result<u16> {
    let (start, end) = configured_port_range()?;

    for _ in 0..PORT_ALLOCATION_ATTEMPTS {
        let port = thread_rng().gen_range(start..end);
        let mut reserved_ports = RESERVED_PORTS
            .lock()
            .expect("reserved port registry mutex poisoned");
        if reserved_ports.contains(&port) {
            continue;
        }

        // Probe on 0.0.0.0 because Docker later publishes ports on all interfaces.
        let Ok(listener) = TcpListener::bind(("0.0.0.0", port)) else {
            continue;
        };
        drop(listener);

        reserved_ports.insert(port);
        return Ok(port);
    }

    Err(anyhow!(
        "failed to allocate an available port in range {start}..{end} after {PORT_ALLOCATION_ATTEMPTS} attempts"
    ))
}

fn configured_port_range() -> Result<(u16, u16)> {
    let start = std::env::var(PORT_RANGE_START_ENV)
        .ok()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(DEFAULT_PORT_RANGE_START);
    let end = std::env::var(PORT_RANGE_END_ENV)
        .ok()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(DEFAULT_PORT_RANGE_END);

    if start >= end {
        return Err(anyhow!(
            "{PORT_RANGE_START_ENV} ({start}) must be lower than {PORT_RANGE_END_ENV} ({end})"
        ));
    }

    if start < 1024 {
        return Err(anyhow!(
            "{PORT_RANGE_START_ENV} ({start}) must be at least 1024"
        ));
    }

    Ok((start, end))
}

#[cfg(test)]
fn default_port_range() -> (u16, u16) {
    (DEFAULT_PORT_RANGE_START, DEFAULT_PORT_RANGE_END)
}

pub fn get_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .next()
        .expect("Failed to find workspace root")
        .to_path_buf()
}

/// Get citrea path from `CITREA_E2E_TEST_BINARY` env
pub fn get_citrea_path() -> Result<PathBuf> {
    std::env::var("CITREA_E2E_TEST_BINARY")
        .map(PathBuf::from)
        .map_err(|_| anyhow!("CITREA_E2E_TEST_BINARY is not set. Cannot resolve citrea path"))
}

#[cfg(feature = "clementine")]
pub fn get_clementine_path() -> Result<PathBuf> {
    std::env::var(CLEMENTINE_ENV)
        .map(PathBuf::from)
        .map_err(|_| {
            anyhow!("CLEMENTINE_E2E_TEST_BINARY is not set. Cannot resolve clementine path")
        })
}

/// Get genesis path from resources
/// TODO: assess need for customable genesis path in e2e tests
pub fn get_default_genesis_path() -> PathBuf {
    let mut path = get_workspace_root();
    path.push("resources");
    path.push("genesis");
    path.push("bitcoin-regtest");
    path
}

pub fn get_genesis_path(base_dir: &Path) -> String {
    let parent = base_dir.parent().expect("Couldn't get parent dir");
    let genesis_path = parent.join("genesis");
    if genesis_path.exists() {
        genesis_path.display().to_string()
    } else {
        get_genesis_path(parent)
    }
}

pub fn generate_test_id() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}

pub fn copy_directory(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let file_name = entry.file_name();
        let src_path = src.join(&file_name);
        let dst_path = dst.join(&file_name);

        if ty.is_dir() {
            copy_directory(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

pub fn tail_file(path: &Path, lines: usize) -> Result<()> {
    println!("tailing path : {path:?}");
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut last_lines = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if last_lines.len() >= lines {
            last_lines.remove(0);
        }

        last_lines.push(line);
    }

    for line in last_lines {
        println!("{line}");
    }

    Ok(())
}

pub async fn wait_for_tcp_bound(host: &str, port: u16, timeout: Option<Duration>) -> Result<()> {
    let timeout = timeout.unwrap_or(Duration::from_secs(30));
    let start = Instant::now();

    while start.elapsed() < timeout {
        if (TcpStream::connect(format!("{host}:{port}")).await).is_ok() {
            debug!("Postgres is accepting connections");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    anyhow::bail!("Failed to connect to {host}:{port} within the specified timeout")
}

pub async fn create_optional<T>(
    pred: bool,
    f: impl Future<Output = Result<T>>,
) -> Result<Option<T>> {
    if pred {
        Ok(Some(f.await?))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{default_port_range, get_available_port};

    #[test]
    fn get_available_port_does_not_reuse_ports_within_process() {
        let mut ports = HashSet::new();
        let (start, end) = default_port_range();

        for _ in 0..32 {
            let port = get_available_port().expect("available port");
            assert!(ports.insert(port), "duplicate allocated port: {port}");
            assert!(
                (start..end).contains(&port),
                "allocated port {port} outside default test range {start}..{end}"
            );
        }
    }
}
