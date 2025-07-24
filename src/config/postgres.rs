use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use crate::{log_provider::LogPathProvider, node::NodeKind};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PostgresConfig {
    pub port: u16,
    pub user: String,
    pub password: String,
    pub log_dir: PathBuf,
    pub extra_args: Vec<String>,
    /// Docker image tag (defaults to `15` for `postgres:15`)
    pub image_tag: Option<String>,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            port: 5432,
            user: "clementine".to_string(),
            password: "clementine".to_string(),
            log_dir: TempDir::new()
                .expect("Failed to create temporary directory")
                .keep(),
            extra_args: vec!["-c".to_string(), "max_connections=1000".to_string()],
            image_tag: None,
        }
    }
}

impl LogPathProvider for PostgresConfig {
    fn kind(&self) -> NodeKind {
        NodeKind::Postgres
    }

    fn log_path(&self) -> PathBuf {
        self.log_dir.join("debug.log")
    }

    fn stderr_path(&self) -> PathBuf {
        self.log_dir.join("stderr.log")
    }
}
