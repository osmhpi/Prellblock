//! Consensus abstractions

use pinxit::Signed;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

mod block;
mod block_number;
mod leader_term;
mod signature_list;

pub use block::{Block, BlockHash, Body};
pub use block_number::BlockNumber;
pub use leader_term::LeaderTerm;
pub use signature_list::SignatureList;

/// The first block in the chain, just a list of `Transaction`s.
#[derive(Debug, Serialize, Deserialize)]
pub struct GenesisTransactions {
    /// The transactions in the genesis block.
    pub transactions: Vec<Signed<super::Transaction>>,
    /// The timestamp of genesis block creation.
    pub timestamp: SystemTime,
}
