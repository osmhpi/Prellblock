#![allow(clippy::module_name_repetitions)]
// This is needed because of a false positive in clippy:
// See [Known problems](https://rust-lang.github.io/rust-clippy/master/#empty_line_after_outer_attr).
#![allow(clippy::empty_line_after_outer_attr)]

#[allow(clippy::module_inception)] // lol :D
mod message;
mod request;
pub mod response;
mod signable;

pub use message::{consensus_message, ConsensusMessage};
pub use request::Request;
pub use response::{consensus_response, ConsensusResponse};

use super::Error;
use crate::consensus::{BlockHash, BlockNumber, LeaderTerm};
use serde::{Deserialize, Serialize};

/// Metadata about a block specific message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    /// The current number of the view (selected leader).
    pub leader_term: LeaderTerm,
    /// The current block number (block height) of this round.
    pub block_number: BlockNumber,
    /// The hash of this rounds block.
    pub block_hash: BlockHash,
}

impl Metadata {
    pub fn verify(&self, other: &Self) -> Result<(), Error> {
        if self == other {
            Ok(())
        } else {
            Err(Error::AckDoesNotMatch)
        }
    }
}
