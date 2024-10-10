use std::path::PathBuf;

use crate::{config::FullVerifierConfig, node::Node, traits::NodeT};

pub type Verifier = Node<FullVerifierConfig>;

impl Verifier {
    pub fn dir(&self) -> &PathBuf {
        &self.config().dir
    }
}
