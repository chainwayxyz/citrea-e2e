// Config imported as is from `citrea` repository `node-config` directory.
// This is done in order not to have cyclical dependencies with `citrea`.
// Should ideally be automatically kept in sync somehow but manually copied here for the time being.
// Configs are stable and not expected to change much.

pub(crate) mod batch_prover;
pub(crate) mod bitcoin;
pub(crate) mod light_client_prover;
pub(crate) mod rollup;
pub(crate) mod sequencer;

#[cfg(test)]
/// Reads toml file as a specific type.
pub fn from_toml_path<P: AsRef<std::path::Path>, R: serde::de::DeserializeOwned>(
    path: P,
) -> anyhow::Result<R> {
    use std::{fs::File, io::Read};

    let mut contents = String::new();
    {
        let mut file = File::open(path)?;
        file.read_to_string(&mut contents)?;
    }
    tracing::debug!("Config file size: {} bytes", contents.len());
    tracing::trace!("Config file contents: {}", &contents);

    let result: R = toml::from_str(&contents)?;

    Ok(result)
}
