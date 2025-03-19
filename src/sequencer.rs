use crate::{config::SequencerConfig, node::Node};

pub type Sequencer = Node<SequencerConfig>;

impl Sequencer {
    pub fn min_l2_blocks_per_commitment(&self) -> u64 {
        self.config.node.min_l2_blocks_per_commitment
    }
}
