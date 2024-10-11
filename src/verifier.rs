use std::path::PathBuf;

use crate::{clementine, config::FullVerifierConfig, traits::NodeT};

pub type Verifier = clementine::node::ClementineNode<FullVerifierConfig>;

impl Verifier {
    pub fn dir(&self) -> &PathBuf {
        &self.config().dir
    }
}
