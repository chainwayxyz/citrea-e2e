use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader},
    net::TcpListener,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{anyhow, bail};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use tokio::{net::TcpStream, time::Instant};
use tracing::debug;

use crate::test_case::CLEMENTINE_ENV;

use super::Result;

pub fn get_available_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
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
    base_dir
        .parent()
        .expect("Couldn't get parent dir")
        .join("genesis")
        .display()
        .to_string()
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
        if let Ok(_) = TcpStream::connect(format!("{host}:{port}")).await {
            debug!("Postgres is accepting connections");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    bail!("Failed to connect to {host}:{port} within the specified timeout")
}
