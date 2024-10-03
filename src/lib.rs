mod bitcoin;
mod bridge_backend;
pub mod client;
pub mod config;
mod docker;
pub mod framework;
mod full_node;
pub mod node;
mod prover;
mod sequencer;
pub mod test_case;
pub mod traits;

mod utils;

pub type Result<T> = anyhow::Result<T>;
