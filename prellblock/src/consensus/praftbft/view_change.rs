use super::{
    message::ConsensusMessage,
    state::{FollowerState, ViewPhase, ViewPhaseMeta},
    Error, PRaftBFT, ViewChangeSignatures,
};
use crate::{
    consensus::{LeaderTerm, SignatureList},
    BoxError,
};
use pinxit::{PeerId, Signature};
use std::{collections::HashMap, time::Duration};
use tokio::sync::MutexGuard;

impl PRaftBFT {
    pub(super) async fn handle_new_view(
        &self,
        peer_id: &PeerId,
        leader_term: LeaderTerm,
        view_change_signatures: SignatureList,
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
        self.verify_rpu_majority_signatures(&view_change_message, &view_change_signatures)?;

        self.new_view_sender
            .broadcast((leader_term, view_change_signatures))
            .unwrap();
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

        let view_change_message = ConsensusMessage::ViewChange { new_leader_term };
        match peer_id.verify(&view_change_message, &signature) {
            Ok(()) => {}
            Err(err) => {
                log::error!("Received invalid view change signature: {}", err);
                return Err(err.into());
            }
        };

        log::trace!(
            "Received leader term {} on current leader term {}.",
            new_leader_term,
            follower_state.leader_term
        );
        let phase = follower_state.view_phase_mut(new_leader_term)?;
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
                let messages = &mut meta.messages;
                messages.insert(peer_id, signature);
                // if enough collected, broadcast message and update state accordingly
                if self.nonfaulty_reached(messages.len()) {
                    let messages = messages.clone();
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
                }
            }

            // if 2f + 1 Signatures collected, start ViewChange timer
            ViewPhase::ViewChanging(meta) => {
                let messages = &mut meta.messages;
                messages.insert(peer_id, signature);
                if self.supermajority_reached(messages.len()) {
                    log::trace!("Supermajority reached.");
                    let messages = messages.iter().collect();
                    follower_state.set_view_phase(new_leader_term, ViewPhase::Changed)?;
                    drop(follower_state);

                    // If this is the leader broadcast wake up leader task.
                    if self.is_current_leader(new_leader_term, self.peer_id()) {
                        // Notify leader task to begin to work.
                        self.enough_view_changes_sender
                            .broadcast((new_leader_term, messages))
                            .expect("running leader task");
                    } else {
                        self.enough_view_changes_sender
                            .broadcast((new_leader_term, SignatureList::default()))
                            .expect("running leader task");
                    }

                    let mut new_view_data = self.new_view_receiver.borrow().clone();
                    let mut new_view_receiver = self.new_view_receiver.clone();
                    let timeout_result = tokio::time::timeout(Duration::from_millis(5000), async {
                        while new_view_data.0 < new_leader_term {
                            new_view_data = new_view_receiver.recv().await.unwrap();
                        }
                    })
                    .await;

                    if new_view_data.0 >= new_leader_term {
                        let (leader_term, new_view_signatures) = new_view_data;
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
                        follower_state.new_view_signatures = new_view_signatures;
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

    /// Send a `ConsensusMessage::ViewChange` message because the leader
    /// seems to be faulty.
    pub(super) async fn request_view_change(
        &self,
        mut follower_state: MutexGuard<'_, FollowerState>,
    ) {
        let requested_new_leader_term = follower_state.leader_term + 1;
        let messages = HashMap::new();
        match follower_state.set_view_phase(
            requested_new_leader_term,
            ViewPhase::ViewChanging(ViewPhaseMeta { messages }),
        ) {
            Ok(()) => {}
            Err(err) => log::error!("Error setting view change phase: {}", err),
        };
        // This drop is needed because receiving messages while
        // broadcasting also requires the lock.
        drop(follower_state);

        match self.broadcast_view_change(requested_new_leader_term).await {
            Ok(()) => {}
            Err(err) => log::error!("Error broadcasting ViewChange: {}", err),
        }
    }
}
