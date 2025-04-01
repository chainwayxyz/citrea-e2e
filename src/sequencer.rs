use crate::{config::SequencerConfig, node::Node};

pub type Sequencer = Node<SequencerConfig>;

impl Sequencer {
    pub fn max_l2_blocks_per_commitment(&self) -> u64 {
        self.config.node.max_l2_blocks_per_commitment
    }
}
