use super::{
    message::ConsensusMessage,
    state::{ViewPhase, ViewPhaseMeta},
    Error, PRaftBFT, ViewChangeSignatures,
};
use crate::{consensus::LeaderTerm, BoxError};
use pinxit::{PeerId, Signature};
use std::{collections::HashMap, time::Duration};

impl PRaftBFT {
    pub(super) async fn handle_new_view(
        &self,
        peer_id: &PeerId,
        leader_term: LeaderTerm,
        view_change_signatures: HashMap<PeerId, Signature>,
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

        self.new_view_sender.broadcast(leader_term).unwrap();
        self.leader_notifier.notify();

        Ok(ConsensusMessage::AckNewView)
    }

    pub(super) async fn broadcast_view_change(
        &self,
        new_leader_term: LeaderTerm,
    ) -> Result<(), Error> {
        log::trace!("Broadcasting ViewChange Message.");
        let view_change_message = ConsensusMessage::ViewChange { new_leader_term };
        let validate_ack_view_change = move |response: &ConsensusMessage| {
            // This is done for every ACKCOMMIT.
            match response {
                ConsensusMessage::AckViewChange => Ok(()),
                _ => Err("This is not an ack ViewChange message.".into()),
            }
        };
        match self
            .broadcast_until_majority(view_change_message, validate_ack_view_change)
            .await
        {
            Ok(_) => log::info!("ViewChange Message Broadcast did reach supermajority."),
            // Malte TM
            Err(err) => log::warn!(
                "ViewChange Message Broadcast did not reach supermajority: {}",
                err
            ),
        };
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) async fn handle_view_change(
        &self,
        peer_id: PeerId,
        signature: Signature,
        new_leader_term: LeaderTerm,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state.lock().await;
        // Only higher leader terms than current one accepted
        if new_leader_term <= follower_state.leader_term {
            return Err(Error::LeaderTermTooSmall(new_leader_term));
        }

        log::trace!(
            "Received leader term {} on current leader term {}.",
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
                    "Changed State to ViewReceiving for Leader Term {}.",
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
                        "Changed to ViewChanging State for Leader Term {}.",
                        new_leader_term
                    );
                    drop(follower_state);
                    self.broadcast_view_change(new_leader_term).await?;
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
                    log::trace!("Supermajority reached.");
                    follower_state.set_view_phase(new_leader_term, ViewPhase::Changed)?;
                    drop(follower_state);

                    // If this is the leader broadcast wake up leader task.
                    if self.is_current_leader(new_leader_term, self.peer_id()) {
                        // Notify leader task to begin to work.
                        self.enough_view_changes_sender
                            .broadcast((
                                new_leader_term,
                                messages
                                    .iter()
                                    .map(|(k, v)| (k.clone(), v.clone()))
                                    .collect(),
                            ))
                            .expect("running leader task");
                    } else {
                        self.enough_view_changes_sender
                            .broadcast((new_leader_term, HashMap::new()))
                            .expect("running leader task");
                    }

                    let mut view_changed = *self.new_view_receiver.borrow();
                    let mut new_view_receiver = self.new_view_receiver.clone();
                    let timeout_result = tokio::time::timeout(Duration::from_millis(5000), async {
                        while view_changed < new_leader_term {
                            view_changed = new_view_receiver.recv().await.unwrap();
                        }
                    })
                    .await;

                    if view_changed >= new_leader_term {
                        let leader_term = view_changed;
                        log::trace!("NewView arrived in time.");

                        let mut follower_state = self.follower_state.lock().await;
                        // The ViewChange was successfull:
                        // Update the leader_term of the follower_state to the new leaderterm
                        log::debug!(
                            "Changed from leader term {} to leader term {}.",
                            follower_state.leader_term,
                            leader_term
                        );
                        follower_state.leader_term = leader_term;
                        follower_state
                            .view_state
                            .increment_to(leader_term, ViewPhase::Waiting);

                        // On view change, we need to drop all messages from the
                        // old leader to allow the new one to send new messages.
                        follower_state.reset_future_round_states();
                    } else {
                        assert_ne!(timeout_result, Ok(()));
                        log::trace!("NewView has not arrived in time.");

                        // resend ViewChange for v + 1
                        self.broadcast_view_change(new_leader_term + 1).await?
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

    pub(super) async fn send_new_view(
        &self,
        leader_term: LeaderTerm,
        signatures: ViewChangeSignatures,
    ) -> Result<(), BoxError> {
        // let mut sigs = vec![];
        // for (key, val) in &messages {
        //     sigs.push((key.clone(), val.clone()));
        // }
        let new_view_message = ConsensusMessage::NewView {
            leader_term,
            view_change_signatures: signatures,
        };
        let validate_new_view = move |response: &ConsensusMessage| {
            // This is done for every ACKCOMMIT.
            match response {
                ConsensusMessage::AckNewView => Ok(()),
                _ => Err("This is not an ack NewView message.".into()),
            }
        };
        self.broadcast_until_majority(new_view_message, validate_new_view)
            .await?;
        Ok(())
    }
}
