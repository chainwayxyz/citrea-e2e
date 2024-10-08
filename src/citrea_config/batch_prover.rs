use serde::{Deserialize, Serialize};

/// The possible configurations of the prover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProverGuestRunConfig {
    /// Skip proving.
    Skip,
    /// Run the rollup verification logic inside the current process.
    Simulate,
    /// Run the rollup verifier in a zkVM executor.
    Execute,
    /// Run the rollup verifier and create a SNARK of execution.
    Prove,
}

impl<'de> Deserialize<'de> for ProverGuestRunConfig {
    fn deserialize<D>(deserializer: D) -> Result<ProverGuestRunConfig, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "skip" => Ok(ProverGuestRunConfig::Skip),
            "simulate" => Ok(ProverGuestRunConfig::Simulate),
            "execute" => Ok(ProverGuestRunConfig::Execute),
            "prove" => Ok(ProverGuestRunConfig::Prove),
            _ => Err(serde::de::Error::custom("invalid prover guest run config")),
        }
    }
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
    use std::io::Write;

    use serde::de::DeserializeOwned;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;
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
