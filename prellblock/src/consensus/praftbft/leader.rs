use super::{
    super::Body, flatten_vec::FlattenVec, message::ConsensusMessage, state::LeaderState,
    MAX_TRANSACTIONS_PER_BLOCK,
};
use crate::{
    peer::{message as peer_message, Sender},
    thread_group::ThreadGroup,
    BoxError,
};
use pinxit::{Identity, PeerId, Signable, Signature, Signed};
use prellblock_client_api::Transaction;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{mpsc, Arc, Mutex},
};

pub(super) struct Leader {
    /// The identity is used to sign consensus messages.
    pub(super) identity: Identity,
    /// A queue of messages.
    pub(super) queue: Arc<Mutex<FlattenVec<Signed<Transaction>>>>,
    pub(super) peers: HashMap<PeerId, SocketAddr>,
    pub(super) leader_state: LeaderState,
}

impl Leader {
    fn broadcast_until_majority<F>(
        &self,
        message: ConsensusMessage,
        verify_response: F,
    ) -> Result<Vec<(PeerId, Signature)>, BoxError>
    where
        F: Fn(&ConsensusMessage) -> Result<(), BoxError> + Clone + Send + 'static,
    {
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
                    let response =
                        send_message_and_verify_response().map_err(|err| (peer_address, err));
                    let _ = tx.send(response);
                },
            );
        }

        // IMPORTANT: when we do not drop this tx, the loop below will loop forever
        drop(tx);

        let mut responses = Vec::new();

        for result in rx {
            match result {
                Ok((peer_id, signature)) => responses.push((peer_id, signature)),
                Err((peer_address, err)) => {
                    log::warn!("Consensus Error from {}: {}", peer_address, err);
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
    // TODO: split this function into smaller phases
    #[allow(clippy::too_many_lines)]
    pub(super) fn process_transactions(mut self) {
        loop {
            // TODO: sleep until timeout
            while self.queue.lock().unwrap().len() < MAX_TRANSACTIONS_PER_BLOCK {
                std::thread::park();
            }

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
                let sequence_number = self.leader_state.sequence + 1;
                let body = Body {
                    height: sequence_number,
                    prev_block_hash: self.leader_state.last_block_hash,
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
                let leader_term = self.leader_state.leader_term;
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
                        log::error!(
                            "Consensus error during PREPARE phase for block #{}: {}",
                            sequence_number,
                            err
                        );
                        // TODO: retry the transactions
                        continue;
                    }
                };
                log::trace!(
                    "Prepare #{} phase ended. Got ACKPREPARE signatures: {:?}",
                    sequence_number,
                    ackprepares,
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
                        log::error!(
                            "Consensus error during APPEND phase for block #{}: {}",
                            sequence_number,
                            err
                        );
                        // TODO: retry the transactions
                        continue;
                    }
                };
                log::trace!(
                    "Append Phase #{} ended. Got ACKAPPEND signatures: {:?}",
                    sequence_number,
                    ackappends,
                );

                // after we collected enough signatures, we can update our state
                self.leader_state.sequence = sequence_number;
                self.leader_state.last_block_hash = hash;

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
                let ackcommits = self.broadcast_until_majority(commit_message, validate_ackcommits);
                match ackcommits {
                    Ok(_) => {
                        log::info!("Comitted block #{} on majority of RPUs.", sequence_number);
                    }
                    Err(err) => {
                        log::error!(
                            "Consensus error during COMMIT phase for block #{}: {}",
                            sequence_number,
                            err
                        );
                    }
                }
            }
        }
    }

    /// Check whether a number represents a supermajority (>2/3) compared
    /// to the peers in the consenus.
    fn supermajority_reached(&self, number: usize) -> bool {
        let len = self.peers.len();
        if len < 4 {
            panic!("Cannot find consensus for less than four peers.");
        }
        let supermajority = len * 2 / 3 + 1;
        number >= supermajority
    }
}
