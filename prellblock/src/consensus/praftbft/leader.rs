use super::{
    super::Body, message::ConsensusMessage, PRaftBFT, Sleeper, MAX_TRANSACTIONS_PER_BLOCK,
};

impl PRaftBFT {
    /// This function waits until it is triggered to process transactions.
    // TODO: split this function into smaller phases
    #[allow(clippy::too_many_lines)]
    pub(super) fn process_transactions(&self, sleeper: &Sleeper) {
        loop {
            // TODO: sleep until timeout
            sleeper.recv().expect(
                "The consensus died. Stopping processing transaction in background thread.",
            );
            let mut leader_state = self.leader_state.lock().unwrap();

            // Die when we are not the leader.
            if !self.is_current_leader(leader_state.leader_term, self.peer_id()) {
                drop(leader_state);
                continue;
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
                let ackcommits = self.broadcast_until_majority(commit_message, validate_ackcommits);
                match ackcommits {
                    Ok(_) => log::info!("Comitted blocks on majority of RPUs."),
                    Err(err) => {
                        log::error!("Consensus error during COMMIT phase: {}", err);
                    }
                }
            }
        }
    }
}
