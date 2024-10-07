use std::path::PathBuf;

use super::config::FullSequencerConfig;
use crate::node::Node;
use crate::traits::NodeT;

pub type Sequencer = Node<FullSequencerConfig>;

impl Sequencer {
    pub fn dir(&self) -> &PathBuf {
        &self.config().dir
    }

    pub fn min_soft_confirmations_per_commitment(&self) -> u64 {
        self.config().node.min_soft_confirmations_per_commitment
    }
}
