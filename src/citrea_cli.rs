use anyhow::anyhow;
use anyhow::bail;
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

pub struct CitreaCli {
    executable_path: PathBuf,
}

impl CitreaCli {
    pub fn new(env_var_name: &str) -> anyhow::Result<Self> {
        match env::var(env_var_name) {
            Ok(path) => Ok(CitreaCli {
                executable_path: PathBuf::from(path),
            }),
            Err(_) => bail!("Environment variable {} not set", env_var_name),
        }
    }

    pub async fn run(&self, command: &str, args: &[&str]) -> anyhow::Result<String> {
        let output = Command::new(&self.executable_path)
            .arg(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!(
                "Command failed with error: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }
}
