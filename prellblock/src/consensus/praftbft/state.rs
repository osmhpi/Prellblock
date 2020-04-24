//! Contains the states used in the consensus.

use super::{
    super::{BlockHash, Body},
    ring_buffer::RingBuffer,
    Error,
};
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
    pub(super) leader_term: usize,
    pub(super) sequence: u64,
    pub(super) round_state: RingBuffer<Phase>,
    pub(super) view_state: RingBuffer<ViewPhase>,
}

const RING_BUFFER_SIZE: usize = 32;

impl FollowerState {
    /// Create a new `FollowerState`.
    pub(super) fn new() -> Self {
        let mut state = Self {
            leader_term: 0,
            sequence: 0,
            round_state: RingBuffer::new(Phase::Waiting, RING_BUFFER_SIZE, 0),
            view_state: RingBuffer::new(ViewPhase::Waiting, RING_BUFFER_SIZE, 0),
        };

        // TODO: remove this fake genesis block
        let fake_genesis_block_hash = BlockHash::default();
        *state.round_state.get_mut(0).unwrap() = Phase::Committed(fake_genesis_block_hash);

        state
    }

    /// Validate that there is a leader and the received message is from this leader.
    pub(super) fn verify_message_meta(
        &self,
        leader_term: usize,
        sequence_number: u64,
    ) -> Result<(), Error> {
        // We only handle the current leader term.
        if leader_term != self.leader_term {
            log::warn!("Follower is not in the correct Leader term");
            return Err(Error::WrongLeaderTerm);
        }

        // Only process new messages.
        if sequence_number <= self.sequence {
            log::warn!("Current Leader's Sequence number is too small.");
            return Err(Error::SequenceNumberTooSmall);
        }
        Ok(())
    }

    /// Get the `Phase` for the given `sequence` if it exists.
    /// This function is necessary because `ConsensusMessage`s can arrive out of order.
    pub fn phase(&self, sequence: u64) -> Result<&Phase, Error> {
        self.round_state
            .get(sequence)
            .ok_or(Error::SequenceNumberTooBig)
    }

    /// Set the `Phase` for a given `sequence`.
    pub fn set_phase(&mut self, sequence: u64, phase: Phase) -> Result<(), Error> {
        *self
            .round_state
            .get_mut(sequence)
            .ok_or(Error::SequenceNumberTooBig)? = phase;
        Ok(())
    }

    /// Get the `ViewPhase` for the given `leader_term` if it exists.
    /// This function is necessary because `ConsensusMessage`s can arrive out of order.
    pub fn view_phase(&self, leader_term: usize) -> Result<&ViewPhase, Error> {
        self.view_state
            .get(leader_term as u64)
            .ok_or(Error::LeaderTermTooBig)
    }

    /// Set the `ViewPhase` for a given `leader_term`.
    pub fn set_view_phase(&mut self, leader_term: usize, phase: ViewPhase) -> Result<(), Error> {
        *self
            .view_state
            .get_mut(leader_term as u64)
            .ok_or(Error::LeaderTermTooBig)? = phase;
        Ok(())
    }

    fn block_hash(&self, index: u64) -> BlockHash {
        match self.round_state.get(index) {
            Some(Phase::Committed(last_block_hash)) => *last_block_hash,
            _ => unreachable!(),
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

impl LeaderState {
    /// Create a new `LeaderState` from a `follower_state`.
    pub(super) fn new(follower_state: &FollowerState) -> Self {
        // TODO: Error handling with genesis block?
        // if sequence == 0 { genesis block not found } else { you f***d up }
        Self {
            leader_term: follower_state.leader_term,
            sequence: follower_state.sequence,
            last_block_hash: follower_state.last_block_hash(),
        }
    }
}
