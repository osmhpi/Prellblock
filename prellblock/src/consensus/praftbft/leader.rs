use super::{
    super::Body, message::ConsensusMessage, PRaftBFT, Sleeper, MAX_TRANSACTIONS_PER_BLOCK,
};
use crate::{
    peer::{message as peer_message, Sender},
    thread_group::ThreadGroup,
    BoxError,
};
use pinxit::{PeerId, Signable, Signature};
use std::sync::mpsc;

impl PRaftBFT {
    fn broadcast_until_majority<F>(
        &self,
        message: ConsensusMessage,
        verify_response: F,
    ) -> Result<Vec<(PeerId, Signature)>, BoxError>
    where
        F: Fn(&ConsensusMessage) -> Result<(), BoxError> + Clone + Send + 'static,
    {
        let own_id = self.identity.id().clone();
        let message = message.sign(&self.identity)?;
        let signed_message = peer_message::Consensus(message);

        let mut thread_group = ThreadGroup::new();
        let (tx, rx) = mpsc::sync_channel(0);

        for &peer_address in self.peers.values() {
            let signed_message = signed_message.clone();
            let verify_response = verify_response.clone();
            let tx = tx.clone();
            thread_group.spawn(
                &format!("Send consensus message to {}", peer_address),
                move || {
                    let send_message_and_verify_response = || {
                        let mut sender = Sender::new(peer_address);
                        let response = sender.send_request(signed_message)?;
                        let signer = response.signer().clone();
                        let verified_response = response.verify()?;
                        verify_response(&*verified_response)?;
                        Ok::<_, BoxError>((signer, verified_response.signature().clone()))
                    };

                    // The rx-side is closed when we probably collected enough signatures.
                    let _ = tx.send(send_message_and_verify_response());
                },
            );
        }

        // IMPORTANT: when we do not drop this tx, the loop below will loop forever
        drop(tx);

        let mut responses = Vec::new();

        for result in rx {
            match result {
                Ok((peer_id, signature)) => responses.push((peer_id, signature)),
                Err(err) => {
                    log::warn!("Consensus Error: {}", err);
                }
            }
            if self.supermajority_reached(responses.len()) {
                // TODO: once async io is used, drop the unused threads
                return Ok(responses);
            }
        }

        // All sender threads have died **before reaching supermajority**.
        Err("Could not get supermajority.".into())
    }

    /// This function waits until it is triggered to process transactions.
    pub(super) fn process_transactions(&self, sleeper: &Sleeper) {
        loop {
            // TODO: sleep until timeout
            sleeper.recv().expect(
                "The consensus died. Stopping processing transaction in background thread.",
            );

            let leader_state = match &self.leader_state {
                Some(leader_state) => leader_state,
                None => {
                    // log::trace!("I am not a leader. Let me sleep.");
                    continue;
                }
            };
            // TODO: Remove this.
            // Die when we are not the leader.
            assert_eq!(
                Some(self.identity.id()),
                self.follower_state.lock().unwrap().leader.as_ref()
            );

            // TODO: use > 0 instead, when in timeout
            while self.queue.lock().unwrap().len() >= MAX_TRANSACTIONS_PER_BLOCK {
                let mut transactions = Vec::with_capacity(MAX_TRANSACTIONS_PER_BLOCK);

                // TODO: Check size of transactions cumulated.
                while let Some(transaction) = self.queue.lock().unwrap().next() {
                    // pack block
                    // TODO: Validate transaction.

                    transactions.push(transaction);

                    if transactions.len() >= MAX_TRANSACTIONS_PER_BLOCK {
                        break;
                    }
                }
                let mut leader_state = self.leader_state.as_ref().unwrap().lock().unwrap();
                let sequence_number = leader_state.sequence + 1;
                let body = Body {
                    height: sequence_number,
                    prev_block_hash: leader_state.last_block_hash,
                    transactions,
                };
                let hash = body.hash();

                let transactions = body.transactions;

                // ----------------------------------------- //
                //    _____                                  //
                //   |  __ \                                 //
                //   | |__) | __ ___ _ __   __ _ _ __ ___    //
                //   |  ___/ '__/ _ \ '_ \ / _` | '__/ _ \   //
                //   | |   | | |  __/ |_) | (_| | | |  __/   //
                //   |_|   |_|  \___| .__/ \__,_|_|  \___|   //
                //    --------------| |-------------------   //
                //                  |_|                      //
                // ----------------------------------------- //
                let leader_term = leader_state.leader_term;
                let prepare_message = ConsensusMessage::Prepare {
                    leader_term,
                    sequence_number,
                    block_hash: hash,
                };

                let validate_ackprepares = move |response: &ConsensusMessage| {
                    // This is done for every ACKPREPARE.
                    match response {
                        ConsensusMessage::AckPrepare {
                            leader_term: ack_leader_term,
                            sequence_number: ack_sequence_number,
                            block_hash: ack_block_hash,
                        } => {
                            // Check whether the ACKPREPARE is for the same message.
                            if *ack_leader_term == leader_term
                                && *ack_sequence_number == sequence_number
                                && *ack_block_hash == hash
                            {
                                Ok(())
                            } else {
                                Err("This is an invalid ACK PREPARE message.".into())
                            }
                        }
                        _ => Err("This is not an ACK PREPARE message.".into()),
                    }
                };
                let ackprepares =
                    self.broadcast_until_majority(prepare_message, validate_ackprepares);

                let ackprepares = match ackprepares {
                    Ok(ackprepares) => ackprepares,
                    Err(err) => {
                        log::error!("Consensus error during PREPARE phase: {}", err);
                        // TODO: retry the transactions
                        continue;
                    }
                };
                log::trace!(
                    "Prepare phase ended. Got ACKPREPARE signatures: {:?}",
                    ackprepares
                );

                // ------------------------------------------- //
                //                                        _    //
                //       /\                              | |   //
                //      /  \   _ __  _ __   ___ _ __   __| |   //
                //     / /\ \ | '_ \| '_ \ / _ \ '_ \ / _` |   //
                //    / ____ \| |_) | |_) |  __/ | | | (_| |   //
                //   /_/    \_\ .__/| .__/ \___|_| |_|\__,_|   //
                //   ---------| |---| |---------------------   //
                //            |_|   |_|                        //
                // ------------------------------------------- //
                let append_message = ConsensusMessage::Append {
                    leader_term,
                    sequence_number,
                    block_hash: hash,
                    ackprepare_signatures: ackprepares,
                    data: transactions,
                };
                let validate_ackappends = move |response: &ConsensusMessage| {
                    // This is done for every ACKPREPARE.
                    match response {
                        ConsensusMessage::AckAppend {
                            leader_term: ack_leader_term,
                            sequence_number: ack_sequence_number,
                            block_hash: ack_block_hash,
                        } => {
                            // Check whether the ACKPREPARE is for the same message.
                            if *ack_leader_term == leader_term
                                && *ack_sequence_number == sequence_number
                                && *ack_block_hash == hash
                            {
                                Ok(())
                            } else {
                                Err("This is an invalid ACK APPEND message.".into())
                            }
                        }
                        _ => Err("This is not an ack append message.".into()),
                    }
                };
                let ackappends = self.broadcast_until_majority(append_message, validate_ackappends);
                let ackappends = match ackappends {
                    Ok(ackappends) => ackappends,
                    Err(err) => {
                        log::error!("Consensus error during APPEND phase: {}", err);
                        // TODO: retry the transactions
                        continue;
                    }
                };
                log::trace!(
                    "Append Phase ended. Got ACKAPPEND signatures: {:?}",
                    ackappends
                );

                // after we collected enough signatures, we can update our state
                leader_state.sequence = sequence_number;
                leader_state.last_block_hash = hash;

                // ------------------------------------------- //
                //     _____                          _ _      //
                //    / ____|                        (_) |     //
                //   | |     ___  _ __ ___  _ __ ___  _| |_    //
                //   | |    / _ \| '_ ` _ \| '_ ` _ \| | __|   //
                //   | |___| (_) | | | | | | | | | | | | |_    //
                //    \_____\___/|_| |_| |_|_| |_| |_|_|\__|   //
                //   ---------------------------------------   //
                //                                             //
                // ------------------------------------------- //

                let commit_message = ConsensusMessage::Commit {
                    leader_term,
                    sequence_number,
                    block_hash: hash,
                    ackappend_signatures: ackappends,
                };

                let validate_ackcommits = move |response: &ConsensusMessage| {
                    // This is done for every ACKCOMMIT.
                    match response {
                        ConsensusMessage::AckCommit => Ok(()),
                        _ => Err("This is not an ack commit message.".into()),
                    }
                };
                self.broadcast_until_majority(commit_message, validate_ackcommits);
            }
        }
    }
}
