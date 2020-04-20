//! Contains the states used in the consensus.

use super::{super::BlockHash, Error};
use pinxit::PeerId;

#[derive(Default)]
pub(super) struct FollowerState {
    pub(super) leader_term: usize,
    pub(super) sequence: usize,
    pub(super) current_block_hash: BlockHash,
    pub(super) leader: Option<PeerId>,
}

impl FollowerState {
    /// Validate that there is a leader and the received message is from this leader.
    pub(super) fn verify_message_meta(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: usize,
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
}

#[derive(Default)]
pub(super) struct LeaderState {
    pub(super) leader_term: usize,
    pub(super) sequence: usize,
    pub(super) current_block_hash: BlockHash,
}

impl LeaderState {
    pub fn new(follower_state: &FollowerState) -> Self {
        Self {
            leader_term: follower_state.leader_term,
            sequence: follower_state.sequence,
            current_block_hash: follower_state.current_block_hash,
        }
    }
}
