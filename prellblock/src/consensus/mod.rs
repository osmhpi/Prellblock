//! Consensus abstractions

mod block;
mod praftbft;

pub use block::{Block, BlockHash, Body, SequenceNumber};
pub use praftbft::{message::ConsensusMessage, PRaftBFT as Consensus};
