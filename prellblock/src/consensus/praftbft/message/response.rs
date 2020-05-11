use super::Metadata;
use crate::consensus::{Block, LeaderTerm, SignatureList};
use newtype_enum::newtype_enum;
use serde::{Deserialize, Serialize};

/// Responses used for finding a consensus.
#[newtype_enum(variants = "consensus_response")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusResponse {
    /// A `ConsensusMessage` that is a direct answer to `ConsensusMessage::Prepare`.
    /// Only sent if the `view_number` and `block_number` are accepted.
    AckPrepare {
        /// The message metadata.
        metadata: Metadata,
    },

    /// A `ConsensusMessage` signalizing that the `Block` is accepted by the Follower.
    AckAppend {
        /// The message metadata.
        metadata: Metadata,
    },

    /// A Response to a `SynchronizationRequest`.
    SynchronizationResponse {
        /// The `NewView` message the sender is missing.
        new_view: Option<(LeaderTerm, SignatureList)>,
        /// The `Block`s the sender has skipped.
        blocks: Vec<Block>,
    },

    /// An empty response.
    Ok,
}
