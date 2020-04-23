use super::{
    message::ConsensusMessage,
    state::{ViewPhase, ViewPhaseMeta},
    Error, PRaftBFT,
};
use crate::BoxError;
use pinxit::{PeerId, Signature};
use std::collections::HashMap;

impl PRaftBFT {
    pub(super) fn handle_new_view(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        view_change_signatures: Vec<(PeerId, Signature)>,
    ) -> Result<ConsensusMessage, Error> {
        log::trace!("Received NewView Message.");
        // Leader must be the next peer from the peers list.
        if !self.is_current_leader(leader_term, peer_id) {
            log::warn!(
                "ID: {} is not the correct leader for the new term {}.",
                peer_id,
                leader_term
            );
            log::error!(
                "New Leader should be {}",
                self.peers[leader_term % self.peers.len()].0
            );
            return Err(Error::WrongLeader(peer_id.clone()));
        }

        // validate all signatures to ensure the ViewChange ist valid.
        let view_change_message = ConsensusMessage::ViewChange {
            new_leader_term: leader_term,
        };
        for (peer_id, signature) in view_change_signatures {
            peer_id.verify(&view_change_message, &signature)?
        }

        let mut follower_state = self.follower_state.lock().unwrap();
        // The ViewChange was successfull:
        // Update the leader_term of the follower_state to the new leaderterm
        log::debug!(
            "Changed from leader term {} to leader term {}",
            follower_state.leader_term,
            leader_term
        );
        follower_state.leader_term = leader_term;

        // check if this is the current leader
        if self.is_current_leader(leader_term, self.identity.id()) {
            let mut leader_state = self.leader_state.lock().unwrap();
            leader_state.leader_term = follower_state.leader_term;
            leader_state.sequence = follower_state.sequence;
            leader_state.last_block_hash = follower_state.last_block_hash();
        }
        // Stop ViewChange-Timer

        // If fails, resend ViewChange on Timer timeout
        Ok(ConsensusMessage::AckNewView)
    }

    pub(super) fn broadcast_view_change(&self, new_leader_term: usize) -> Result<(), BoxError> {
        log::trace!("Broadcasting ViewChange Message");
        let view_change_message = ConsensusMessage::ViewChange { new_leader_term };
        let validate_ack_view_change = move |response: &ConsensusMessage| {
            // This is done for every ACKCOMMIT.
            match response {
                ConsensusMessage::AckViewChange => Ok(()),
                _ => Err("This is not an ack ViewChange message.".into()),
            }
        };
        let _ = self.broadcast_until_majority(view_change_message, validate_ack_view_change)?;
        Ok(())
    }

    pub(super) fn handle_view_change(
        &self,
        peer_id: PeerId,
        signature: Signature,
        new_leader_term: usize,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state.lock().unwrap();
        // Only higher leader terms than current one accepted
        if new_leader_term <= follower_state.leader_term {
            return Err(Error::LeaderTermTooSmall);
        }

        log::trace!(
            "Received leader term {} on current leader term {}",
            new_leader_term,
            follower_state.leader_term
        );
        let phase = follower_state.view_phase(new_leader_term)?;
        match phase {
            // insert ID + Signature in hashmap for incoming v
            ViewPhase::Waiting => {
                let mut messages = HashMap::new();
                messages.insert(peer_id, signature);
                follower_state.set_view_phase(
                    new_leader_term,
                    ViewPhase::ViewReceiving(ViewPhaseMeta { messages }),
                )?;
                log::trace!(
                    "Changed State to ViewReceiving for Leader Term {}",
                    new_leader_term
                );
            }
            // if f + 1 Signatures collected, broadcast own ViewChange if not already sent
            ViewPhase::ViewReceiving(meta) => {
                let mut messages = meta.messages.clone();
                messages.insert(peer_id, signature);
                // if enough collected, broadcast message and update state accordingly
                if self.nonfaulty_reached(messages.len()) {
                    // TODO: Macro??
                    follower_state.set_view_phase(
                        new_leader_term,
                        ViewPhase::ViewChanging(ViewPhaseMeta { messages }),
                    )?;
                    log::trace!(
                        "Changed to ViewChanging State for Leader Term {}",
                        new_leader_term
                    );
                    self.broadcast_view_change(new_leader_term).unwrap();
                } else {
                    follower_state.set_view_phase(
                        new_leader_term,
                        ViewPhase::ViewReceiving(ViewPhaseMeta { messages }),
                    )?;
                }
            }
            // if 2f + 1 Signatures collected, start ViewChange timer
            ViewPhase::ViewChanging(meta) => {
                let mut messages = meta.messages.clone();
                messages.insert(peer_id, signature);
                if self.supermajority_reached(messages.len()) {
                    //start timer
                    log::trace!("Supermajority reached!");
                    follower_state.set_view_phase(new_leader_term, ViewPhase::Changed)?;
                    drop(follower_state);
                    if self.is_current_leader(new_leader_term, self.identity.id()) {
                        // broadcast
                        let validate_new_view = move |response: &ConsensusMessage| {
                            // This is done for every ACKCOMMIT.
                            match response {
                                ConsensusMessage::AckNewView => Ok(()),
                                _ => Err("This is not an ack NewView message.".into()),
                            }
                        };
                        let mut sigs = vec![];
                        for (key, val) in &messages {
                            sigs.push((key.clone(), val.clone()));
                        }
                        let new_view_message = ConsensusMessage::NewView {
                            leader_term: new_leader_term,
                            view_change_signatures: sigs,
                        };
                        let _ = self.broadcast_until_majority(new_view_message, validate_new_view);
                    } // else start ViewChange timer
                } else {
                    follower_state.set_view_phase(
                        new_leader_term,
                        ViewPhase::ViewChanging(ViewPhaseMeta { messages }),
                    )?;
                }
            }
            ViewPhase::Changed => {}
        }
        Ok(ConsensusMessage::AckViewChange)
    }
}
