use crate::consensus::BlockHash;
use pinxit::{PeerId, Signable, Signature, Signed};
use prellblock_client_api::Transaction;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::module_name_repetitions)]
/// Messages used for finding a consensus.
pub enum ConsensusMessage {
    /// The first `ConsensusMessage`. Checks for Validity of `view_number` and `sequence_number`.
    Prepare {
        /// The current number of the view (selected leader).
        leader_term: usize,
        /// The current sequence number (block height) of this round.
        sequence_number: u64,
        /// The hash of this rounds block.
        block_hash: BlockHash,
    },
    /// A `ConsensusMessage` that is a direct answer to `ConsensusMessage::Prepare`.
    /// Only sent if the `view_number` and `sequence_number` are accepted.
    AckPrepare {
        /// The current number of the view (selected leader).
        leader_term: usize,
        /// The current sequence number (block height) of this round.
        sequence_number: u64,
        /// The hash of this rounds block.
        block_hash: BlockHash,
    },
    /// A `ConsensusMessage` that prepares the followers for the appending of a `Block` to the blockchain.
    Append {
        /// The current number of the view (selected leader).
        leader_term: usize,
        /// The current sequence number (block height) of this round.
        sequence_number: u64,
        /// The hash of this rounds block.
        block_hash: BlockHash,
        /// The signatures of all (2f+1) `AckPrepare` signatures.
        ackprepare_signatures: Vec<(PeerId, Signature)>,
        /// The transactions of the current `Block`.
        ///
        /// This should match the current `block_hash`.
        data: Vec<Signed<Transaction>>,
    },
    /// A `ConsensusMessage` signalizing that the `Block` is accepted by the Follower.
    AckAppend {
        /// The current number of the view (selected leader).
        leader_term: usize,
        /// The current sequence number (block height) of this round.
        sequence_number: u64,
        /// The hash of this rounds block.
        block_hash: BlockHash,
    },
    /// A `ConsensusMessage` signalizing the Followers to Store the Block in the `BlockStorage` together with the `ACKAPPEND`-Signatures.
    Commit {
        /// The current number of the view (selected leader).
        leader_term: usize,
        /// The current sequence number (block height) of this round.
        sequence_number: u64,
        /// The hash of this rounds block.
        block_hash: BlockHash,
        /// The signatures of all (2f+1) `AckAppend` signatures.
        ackappend_signatures: Vec<(PeerId, Signature)>,
    },
    /// A `ConsensusMessage` signalizing the Leader that a Follower has applied the transactions.
    AckCommit,
    /// A `ConsensusMessage` to propose a Leader Change because of faulty behaviour.
    ViewChange {
        /// The Leader Term we want to swap to.
        new_leader_term: usize,
    },
    /// A `ConsensusMessage` signalizing the sender RPU that another RPU received the `ViewChange` message.
    AckViewChange,
    /// A `ConsensusMessage` signalizing that the new leader has accepted their term.
    NewView {
        /// The Leader term we swapped to.
        leader_term: usize,
        /// The ViewChange signatures of 2f + 1 Replicas
        view_change_signatures: Vec<(PeerId, Signature)>,
    },
    /// A `ConsensusMessage` signalizing the sender RPU that another RPU received the `NewView` message.
    AckNewView,
}

impl Signable for ConsensusMessage {
    type SignableData = Vec<u8>;
    type Error = postcard::Error;
    fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
        match self {
            Self::Append {
                leader_term,
                sequence_number,
                block_hash,
                ackprepare_signatures,
                ..
            } => {
                let sign_data = (
                    leader_term,
                    sequence_number,
                    block_hash,
                    ackprepare_signatures,
                );
                postcard::to_stdvec(&sign_data)
            }
            _ => postcard::to_stdvec(self),
        }
    }
}
