pub mod bitcoin;
mod citrea_config;
mod client;
pub mod config;
mod docker;
pub mod framework;
mod log_provider;
pub mod node;
mod sequencer;
pub mod test_case;
pub mod traits;
mod utils;

pub type Result<T> = anyhow::Result<T>;
