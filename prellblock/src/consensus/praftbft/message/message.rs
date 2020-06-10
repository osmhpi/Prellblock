use super::{InvalidTransaction, Metadata};
use crate::consensus::{BlockHash, BlockNumber, LeaderTerm, SignatureList};
use newtype_enum::newtype_enum;
use pinxit::Signed;
use prellblock_client_api::Transaction;
use serde::{Deserialize, Serialize};
use std::{ops::Deref, time::SystemTime};

/// Messages used for finding a consensus.
#[newtype_enum(variants = "consensus_message")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusMessage {
    /// The first `ConsensusMessage`. Checks for Validity of `view_number` and `block_number`.
    Prepare {
        /// The message metadata.
        metadata: Metadata,
    },

    /// A `ConsensusMessage` that prepares the followers for the appending of a `Block` to the blockchain.
    Append {
        /// The message metadata.
        metadata: Metadata,
        /// The signatures of all (2f+1) `AckPrepare` signatures.
        ackprepare_signatures: SignatureList,
        /// The transactions of the current `Block`.
        ///
        /// This should match the current `block_hash`.
        valid_transactions: Vec<Signed<Transaction>>,
        /// Invalid transactions to remove from the follower's queue.
        /// The indices point to the position at which they whould be applied.
        invalid_transactions: Vec<InvalidTransaction>,
        /// The timestamp of when the proposed Block was created by the leader.
        timestamp: SystemTime,
    },

    /// A `ConsensusMessage` signalizing the Followers to Store the Block in the `BlockStorage` together with the `ACKAPPEND`-Signatures.
    Commit {
        /// The message metadata.
        metadata: Metadata,
        /// The signatures of all (2f+1) `AckAppend` signatures.
        ackappend_signatures: SignatureList,
    },

    /// A `ConsensusMessage` to propose a Leader Change because of faulty behaviour.
    ViewChange {
        /// The Leader Term we want to swap to.
        new_leader_term: LeaderTerm,
    },

    /// A `ConsensusMessage` signalizing that the new leader has accepted their term.
    NewView {
        /// The Leader term we swapped to.
        leader_term: LeaderTerm,
        /// The ViewChange signatures of 2f + 1 Replicas.
        view_change_signatures: SignatureList,
        /// The current block number of the leader.
        current_block_number: BlockNumber,
    },

    /// A Request issued during synchronization.
    SynchronizationRequest {
        /// The current leader term of the sender.
        leader_term: LeaderTerm,
        /// The current block number of the sender.
        block_number: BlockNumber,
        /// The block hash of the topmost block we have.
        block_hash: BlockHash,
    },
}

impl Deref for consensus_message::Prepare {
    type Target = Metadata;
    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}

impl Deref for consensus_message::Append {
    type Target = Metadata;
    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}

impl Deref for consensus_message::Commit {
    type Target = Metadata;
    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}
