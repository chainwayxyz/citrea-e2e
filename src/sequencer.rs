use crate::{config::SequencerConfig, node::Node};

pub type Sequencer = Node<SequencerConfig>;

impl Sequencer {
    pub fn min_soft_confirmations_per_commitment(&self) -> u64 {
        self.config.node.min_soft_confirmations_per_commitment
    }
}
