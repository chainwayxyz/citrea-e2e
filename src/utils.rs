use std::{
    fs,
    fs::File,
    io,
    io::{BufRead, BufReader},
    net::TcpListener,
    path::{Path, PathBuf},
};

use rand::{distributions::Alphanumeric, thread_rng, Rng};
use which::which;

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

/// Get citrea path from CITREA env or resolves to PATH using which.
pub fn get_citrea_path() -> PathBuf {
    std::env::var("CITREA").map_or_else(|_| which("citrea").unwrap(), PathBuf::from)
}

pub fn get_stdout_path(dir: &Path) -> PathBuf {
    dir.join("stdout.log")
}

pub fn get_stderr_path(dir: &Path) -> PathBuf {
    dir.join("stderr.log")
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

pub fn get_genesis_path(dir: &Path) -> PathBuf {
    dir.join("genesis")
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
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut last_lines = Vec::with_capacity(lines);

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
