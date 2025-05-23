use serde::{Deserialize, Serialize};

/// The possible configurations of the prover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProverGuestRunConfig {
    /// Skip proving.
    Skip,
    /// Run the rollup verifier in a zkVM executor.
    Execute,
    /// Run the rollup verifier and create a SNARK of execution.
    Prove,
    /// Run the rollup verifier and create a SNARK or a fake proof of execution.
    ProveWithFakes,
}

/// Prover configuration
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BatchProverConfig {
    /// Prover run mode
    pub proving_mode: ProverGuestRunConfig,
    /// Average number of commitments to prove
    pub proof_sampling_number: usize,
    /// If true prover will try to recover ongoing proving sessions
    pub enable_recovery: bool,
}

impl Default for BatchProverConfig {
    fn default() -> Self {
        Self {
            proving_mode: ProverGuestRunConfig::Execute,
            proof_sampling_number: 0,
            enable_recovery: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{Read, Write},
        path::Path,
    };

    use serde::de::DeserializeOwned;
    use tempfile::NamedTempFile;

    use super::*;

    /// Reads toml file as a specific type.
    pub fn from_toml_path<P: AsRef<Path>, R: DeserializeOwned>(path: P) -> anyhow::Result<R> {
        let mut contents = String::new();
        {
            let mut file = File::open(path)?;
            file.read_to_string(&mut contents)?;
        }
        let result: R = toml::from_str(&contents)?;

        Ok(result)
    }

    fn create_config_from(content: &str) -> NamedTempFile {
        let mut config_file = NamedTempFile::new().unwrap();
        config_file.write_all(content.as_bytes()).unwrap();
        config_file
    }

    #[test]
    fn test_correct_prover_config() {
        let config = r#"
            proving_mode = "skip"
            proof_sampling_number = 500
            enable_recovery = true
        "#;

        let config_file = create_config_from(config);

        let config: BatchProverConfig = from_toml_path(config_file.path()).unwrap();
        let expected = BatchProverConfig {
            proving_mode: ProverGuestRunConfig::Skip,
            proof_sampling_number: 500,
            enable_recovery: true,
        };
        assert_eq!(config, expected);
    }
}
