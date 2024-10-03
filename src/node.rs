use std::fmt;

#[derive(Debug)]
pub enum NodeKind {
    Bitcoin,
    BridgeBackend,
    Prover,
    Sequencer,
    FullNode,
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeKind::Bitcoin => write!(f, "bitcoin"),
            NodeKind::BridgeBackend => write!(f, "bridge-backend"),
            NodeKind::Prover => write!(f, "prover"),
            NodeKind::Sequencer => write!(f, "sequencer"),
            NodeKind::FullNode => write!(f, "full-node"),
        }
    }
}
