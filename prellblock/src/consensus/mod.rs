//! Consensus abstractions

mod praftbft;
mod transaction_applier;

pub use praftbft::{
    ConsensusMessage, ConsensusResponse, Error, PRaftBFT as Consensus, Queue, RingBuffer,
};
pub(crate) use prellblock_client_api::consensus::{
    Block, BlockHash, BlockNumber, Body, LeaderTerm, SignatureList,
};
pub use transaction_applier::TransactionApplier;
