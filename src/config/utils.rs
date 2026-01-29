use std::{fmt::Debug, path::Path};

use serde::Serialize;

pub fn config_to_file<C, P>(config: &C, path: &P) -> std::io::Result<()>
where
    C: Serialize + Debug,
    P: AsRef<Path> + Debug,
{
    let toml = toml::to_string(config).map_err(std::io::Error::other)?;
    std::fs::write(path, toml)?;
    Ok(())
}
