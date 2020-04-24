use super::{
    message::ConsensusMessage,
    state::{ViewPhase, ViewPhaseMeta},
    Error, PRaftBFT,
};
use pinxit::{PeerId, Signature};
use std::{collections::HashMap, thread, time::Duration};

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
            return Err(Error::WrongLeader(peer_id.clone()));
        }

        // validate all signatures to ensure the ViewChange ist valid.
        let view_change_message = ConsensusMessage::ViewChange {
            new_leader_term: leader_term,
        };
        for (peer_id, signature) in view_change_signatures {
            peer_id.verify(&view_change_message, &signature)?
        }

        let (lock, cvar) = &*self.view_change_cvar;
        let mut view_changed = lock.lock().unwrap();
        *view_changed = leader_term;
        cvar.notify_one();

        Ok(ConsensusMessage::AckNewView)
    }

    pub(super) fn broadcast_view_change(&self, new_leader_term: usize) -> Result<(), Error> {
        log::trace!("Broadcasting ViewChange Message");
        let view_change_message = ConsensusMessage::ViewChange { new_leader_term };
        let validate_ack_view_change = move |response: &ConsensusMessage| {
            // This is done for every ACKCOMMIT.
            match response {
                ConsensusMessage::AckViewChange => Ok(()),
                _ => Err("This is not an ack ViewChange message.".into()),
            }
        };
        match self.broadcast_until_majority(view_change_message, validate_ack_view_change) {
            Ok(_) => {}
            // Malte TM
            Err(err) => log::warn!(
                "ViewChange Message Broadcast did not reach supermajority: {}",
                err
            ),
        };
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
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
                    self.broadcast_view_change(new_leader_term)?;
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
                    log::trace!("Supermajority reached!");
                    follower_state.set_view_phase(new_leader_term, ViewPhase::Changed)?;
                    let (lock, cvar) = &*self.view_change_cvar.clone();
                    let view_changed = lock.lock().unwrap();
                    drop(follower_state);
                    // If this is the leader broadcast NewView message
                    if self.is_current_leader(new_leader_term, self.peer_id()) {
                        let broadcast_meta = self.broadcast_meta.clone();
                        thread::spawn(move || {
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
                            match broadcast_meta
                                .broadcast_until_majority(new_view_message, validate_new_view)
                            {
                                Ok(_) => log::trace!("Succesfully broadcasted NewView Message"),
                                Err(err) => {
                                    log::warn!("Error while Broadcasting NewView Message: {}", err)
                                }
                            }
                        });
                    }
                    let (view_changed, timeout_result) = cvar
                        .wait_timeout_while(
                            view_changed,
                            Duration::from_millis(5000),
                            |view_changed| *view_changed < new_leader_term,
                        )
                        .unwrap();

                    if *view_changed >= new_leader_term {
                        let leader_term = *view_changed;
                        // NewView arrived in Time
                        log::info!("NewView arrived in time");

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
                        let mut leader_state = self.leader_state.lock().unwrap();
                        leader_state.leader_term = follower_state.leader_term;
                        if self.is_current_leader(leader_term, self.peer_id()) {
                            leader_state.sequence = follower_state.sequence;
                            leader_state.last_block_hash = follower_state.last_block_hash();
                            self.queue.lock().unwrap().clear();
                            self.waker.send(()).unwrap();
                        }
                    } else {
                        assert!(timeout_result.timed_out());
                        log::info!("NewView has not arrived in time");

                        // resend ViewChange for v + 1
                        self.broadcast_view_change(new_leader_term + 1)?
                    }
                } else {
                    // no supermajority
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
