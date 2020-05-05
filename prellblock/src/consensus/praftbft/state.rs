//! Contains the states used in the consensus.

use super::{
    super::{BlockHash, BlockNumber, Body, LeaderTerm},
    message::ConsensusMessage,
    ring_buffer::RingBuffer,
    Error, NewViewSignatures,
};
use crate::world_state::WorldState;
use pinxit::{PeerId, Signature};
use std::collections::HashMap;

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

/// This contains state needed for every block.
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

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub(super) enum ViewPhase {
    /// This phase was never used, waiting for prepare message.
    Waiting,
    /// Received ViewChange, waiting for more or leader failures.
    ViewReceiving(ViewPhaseMeta),
    /// Sent ViewChange, waiting for NewView.
    ViewChanging(ViewPhaseMeta),
    /// Received or sent NewView Message, ignoring all new messages on the same View
    Changed,
}

#[derive(Clone)]
pub(super) struct ViewPhaseMeta {
    /// the `Phase`'s `Leader`'s `PeerId`
    pub(super) messages: HashMap<PeerId, Signature>,
}

pub(super) struct FollowerState {
    pub(super) leader_term: LeaderTerm,
    pub(super) new_view_signatures: NewViewSignatures,
    pub(super) block_number: BlockNumber,
    pub(super) round_states: RingBuffer<RoundState>,
    pub(super) view_state: RingBuffer<ViewPhase>,
}

const RING_BUFFER_SIZE: usize = 1024;

impl FollowerState {
    /// Create a new `FollowerState` from a `world_state`.
    pub(super) fn from_world_state(world_state: &WorldState) -> Self {
        let start = world_state.block_number;
        let mut state = Self {
            leader_term: LeaderTerm::default(),
            new_view_signatures: HashMap::new(),
            block_number: start,
            round_states: RingBuffer::new(RoundState::default(), RING_BUFFER_SIZE, start.into()),
            view_state: RingBuffer::new(ViewPhase::Waiting, RING_BUFFER_SIZE, 0),
        };

        state.round_state_mut(start).unwrap().phase = Phase::Committed(world_state.last_block_hash);

        state
    }

    /// Validate that there is a leader and the received message is from this leader.
    pub(super) fn verify_message_meta(
        &self,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
    ) -> Result<(), Error> {
        // We only handle the current leader term.
        if leader_term != self.leader_term {
            let error = Error::WrongLeaderTerm;
            log::warn!("{}", error);
            return Err(error);
        }

        // Only process new messages.
        if block_number <= self.block_number {
            let error = Error::BlockNumberTooSmall(block_number);
            log::warn!("{}", error);
            return Err(error);
        }
        Ok(())
    }

    /// Get the `RoundState` for the given `block` if it exists.
    /// This function is necessary because `ConsensusMessage`s can arrive out of order.
    pub fn round_state(&self, block_number: BlockNumber) -> Result<&RoundState, Error> {
        self.round_states
            .get(block_number.into())
            .ok_or(Error::BlockNumberTooBig(block_number))
    }

    /// Get the mutable `RoundState` for a given `block`.
    pub fn round_state_mut(&mut self, block_number: BlockNumber) -> Result<&mut RoundState, Error> {
        self.round_states
            .get_mut(block_number.into())
            .ok_or(Error::BlockNumberTooBig(block_number))
    }

    /// Reset the state of the currently ongoing and all future rounds.
    ///
    /// This allows dropping already received messages / state.
    pub(super) fn reset_future_round_states(&mut self) {
        let mut round_to_reset: u64 = (self.block_number + 1).into();
        while let Some(round_state) = self.round_states.get_mut(round_to_reset) {
            *round_state = RoundState::default();
            round_to_reset += 1;
        }
    }

    /// Get the `ViewPhase` for the given `leader_term` if it exists.
    /// This function is necessary because `ConsensusMessage`s can arrive out of order.
    pub fn view_phase(&self, leader_term: LeaderTerm) -> Result<&ViewPhase, Error> {
        self.view_state
            .get(leader_term.into())
            .ok_or(Error::LeaderTermTooBig(leader_term))
    }

    /// Set the `ViewPhase` for a given `leader_term`.
    pub fn set_view_phase(
        &mut self,
        leader_term: LeaderTerm,
        phase: ViewPhase,
    ) -> Result<(), Error> {
        *self
            .view_state
            .get_mut(leader_term.into())
            .ok_or(Error::LeaderTermTooBig(leader_term))? = phase;
        Ok(())
    }

    /// Get the block hash of the previously committed block.
    pub fn last_block_hash(&self) -> BlockHash {
        if let Some(round_state) = self.round_states.get(self.block_number.into()) {
            match &round_state.phase {
                Phase::Committed(last_block_hash) => *last_block_hash,
                _ => unreachable!(),
            }
        } else {
            unreachable!();
        }
    }
}

#[derive(Default)]
pub(super) struct LeaderState {
    pub(super) block: BlockNumber,
    pub(super) last_block_hash: BlockHash,
    pub(super) leader_term: LeaderTerm,
}

impl LeaderState {
    /// Create a new `LeaderState` from a `follower_state`.
    pub(super) fn new(follower_state: &FollowerState) -> Self {
        // TODO: Error handling with genesis block?
        // if block == 0 { genesis block not found } else { you f***d up }
        Self {
            block: follower_state.block_number,
            last_block_hash: follower_state.last_block_hash(),
            leader_term: follower_state.leader_term,
        }
    }
}
