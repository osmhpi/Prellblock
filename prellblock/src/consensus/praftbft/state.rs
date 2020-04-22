//! Contains the states used in the consensus.

use super::{
    super::{BlockHash, Body},
    message::ConsensusMessage,
    ring_buffer::RingBuffer,
    Error,
};
use pinxit::PeerId;

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub(super) enum Phase {
    /// This phase was never used, waiting for prepare message.
    Waiting,
    /// Sent ACKPrepare, waiting for append message.
    Prepare(PhaseMeta),
    /// Sent ACKAppend, waiting for commit message.
    Append(PhaseMeta, Body),
    /// Successfully committed the block.
    Committed(BlockHash),
}

/// This contains state needed for every sequence.
///
/// Enables out-of-order reception of messages.
/// It is possible that `ConsensusMessage::Commit` or `ConsensusMessage::Append`
/// arrive before having received a `ConsensusMessage::Prepare`.
///
/// Therefore the following permutations of reception are ok (provided all signatures are valid):
///
/// 1. `ConsensusMessage::Prepare` -> `ConsensusMessage::Append` -> `ConsensusMessage::Commit`
/// 2. `ConsensusMessage::Append` -> `ConsensusMessage::Commit`
/// 3. `ConsensusMessage::Commit` -> `ConsensusMessage::Append`
///
/// Other `ConsensusMessage::Prepare` messages will be ignored.
#[derive(Clone)]
pub(super) struct RoundState {
    pub(super) buffered_commit_message: Option<ConsensusMessage>,
    pub(super) phase: Phase,
}

impl Default for RoundState {
    fn default() -> Self {
        Self {
            buffered_commit_message: None,
            phase: Phase::Waiting,
        }
    }
}

#[derive(Clone)]
pub(super) struct PhaseMeta {
    /// the `Phase`'s `Leader`'s `PeerId`
    pub(super) leader: PeerId,
    /// the `BlockHash` of the current `Block`
    pub(super) block_hash: BlockHash,
}

pub(super) struct FollowerState {
    pub(super) leader_term: usize,
    pub(super) leader: Option<PeerId>,
    pub(super) sequence: u64,
    pub(super) round_states: RingBuffer<RoundState>,
}

const RING_BUFFER_SIZE: usize = 32;

impl FollowerState {
    /// Create a new `FollowerState`.
    pub(super) fn new() -> Self {
        let mut state = Self {
            leader_term: 0,
            leader: None,
            sequence: 0,
            round_states: RingBuffer::new(RoundState::default(), RING_BUFFER_SIZE, 0),
        };

        // TODO: remove this fake genesis block
        let fake_genesis_block_hash = BlockHash::default();
        state.round_state_mut(0).unwrap().phase = Phase::Committed(fake_genesis_block_hash);

        state
    }

    /// Validate that there is a leader and the received message is from this leader.
    pub(super) fn verify_message_meta(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: u64,
    ) -> Result<(), Error> {
        // We only handle the current leader term.
        if leader_term != self.leader_term {
            log::warn!("Follower is not in the correct Leader term");
            return Err(Error::WrongLeaderTerm);
        }

        // There should be a known leader.
        let leader = if let Some(leader) = &self.leader {
            leader
        } else {
            // TODO: Trigger leader fetch or election?
            log::warn!("No current leader set");
            return Err(Error::NoLeader);
        };

        // Leader must be the same as we know for the current leader term.
        if leader != peer_id {
            log::warn!(
                "Received Prepare message from invalid leader (ID: {}).",
                peer_id
            );
            return Err(Error::WrongLeader(peer_id.clone()));
        }

        // Only process new messages.
        if sequence_number <= self.sequence {
            log::warn!("Current Leader's Sequence number is too small.");
            return Err(Error::SequenceNumberTooSmall);
        }
        Ok(())
    }

    /// Get the `RoundState` for the given `sequence` if it exists.
    /// This function is necessary because `ConsensusMessage`s can arrive out of order.
    pub fn round_state(&self, sequence: u64) -> Result<&RoundState, Error> {
        self.round_states
            .get(sequence)
            .ok_or(Error::SequenceNumberTooBig)
    }

    /// Set the `RoundState` for a given `sequence`.
    pub fn round_state_mut(&mut self, sequence: u64) -> Result<&mut RoundState, Error> {
        self.round_states
            .get_mut(sequence)
            .ok_or(Error::SequenceNumberTooBig)
    }

    fn block_hash(&self, index: u64) -> BlockHash {
        if let Some(round_state) = self.round_states.get(index) {
            match &round_state.phase {
                Phase::Committed(last_block_hash) => *last_block_hash,
                _ => unreachable!(),
            }
        } else {
            unreachable!();
        }
    }

    /// Get the block hash of the previously committed block.
    pub fn last_block_hash(&self) -> BlockHash {
        self.block_hash(self.sequence)
    }
}

#[derive(Default)]
pub(super) struct LeaderState {
    pub(super) leader_term: usize,
    pub(super) sequence: u64,
    pub(super) last_block_hash: BlockHash,
}

// impl LeaderState {
//     /// Create a new `LeaderState` from a `follower_state`.
//     pub(super) fn new(follower_state: &FollowerState) -> Self {
//         // TODO: Error handling with genesis block?
//         // if sequence == 0 { genesis block not found } else { you f***d up }
//         Self {
//             leader_term: follower_state.leader_term,
//             sequence: follower_state.sequence,
//             last_block_hash: follower_state.last_block_hash(),
//         }
//     }
// }
