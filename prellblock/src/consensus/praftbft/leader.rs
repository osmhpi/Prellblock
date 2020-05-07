use super::{
    super::{Body, LeaderTerm},
    message::ConsensusMessage,
    state::LeaderState,
    PRaftBFT, ViewChangeSignatures, MAX_TRANSACTIONS_PER_BLOCK,
};
use pinxit::Signed;
use prellblock_client_api::Transaction;
use std::{ops::Deref, sync::Arc, time::Duration};
use tokio::{
    sync::{watch, Notify},
    time::timeout,
};

const BLOCK_GENERATION_TIMEOUT: Duration = Duration::from_millis(400);

pub(super) struct Leader {
    pub(super) praftbft: Arc<PRaftBFT>,
    pub(super) leader_state: LeaderState,
}

impl Deref for Leader {
    type Target = PRaftBFT;
    fn deref(&self) -> &Self::Target {
        &self.praftbft
    }
}

impl Leader {
    /// This function waits until it is triggered to process transactions.
    // TODO: split this function into smaller phases
    #[allow(clippy::too_many_lines)]
    pub(super) async fn process_transactions(
        mut self,
        notifier: Arc<Notify>,
        mut enough_view_changes_receiver: watch::Receiver<(LeaderTerm, ViewChangeSignatures)>,
    ) {
        loop {
            // Wait when we are not the leader.
            let mut view_change_signatures = None;
            let mut leader_term = self
                .leader_state
                .leader_term
                .max(enough_view_changes_receiver.borrow().0);
            while !self.is_current_leader(leader_term, self.peer_id()) {
                log::trace!("I am not the current leader.");
                let new_data = enough_view_changes_receiver.recv().await.unwrap();
                leader_term = new_data.0;
                view_change_signatures = Some(new_data.1);
            }

            // Update leader state with data from the follower state when we are the new leader.
            if let Some(view_change_signatures) = view_change_signatures {
                let (block_number, last_block_hash) = {
                    let follower_state = self.follower_state.lock().await;
                    (
                        follower_state.block_number,
                        follower_state.last_block_hash(),
                    )
                };
                log::info!(
                    "I am the new leader in view {} (last block: #{}).",
                    leader_term,
                    block_number
                );

                // Send new view message.
                match self
                    .send_new_view(leader_term, view_change_signatures, block_number)
                    .await
                {
                    Ok(_) => log::trace!("Succesfully broadcasted NewView Message."),
                    Err(err) => {
                        log::warn!("Error while Broadcasting NewView Message: {}", err);
                        // After not reaching the majority, we need to wait until the next time
                        // we are elected. (At least one round later).
                        self.leader_state.leader_term = leader_term + 1;
                        continue;
                    }
                }

                self.leader_state.leader_term = leader_term;
                self.leader_state.block_number = block_number;
                self.leader_state.last_block_hash = last_block_hash;
            }

            let timeout_result = timeout(BLOCK_GENERATION_TIMEOUT, notifier.notified()).await;

            let minimum_queue_len = match timeout_result {
                Ok(()) => MAX_TRANSACTIONS_PER_BLOCK,
                // We want to propose a block with a minimum of 1 transaction
                // when timed out.
                Err(_) => 1,
            };

            while self.queue.read().await.len() >= minimum_queue_len {
                let mut transactions: Vec<Signed<Transaction>> = Vec::new();

                // TODO: Check size of transactions cumulated.
                while let Some((_, transaction)) = self.queue.write().await.pop_front() {
                    // pack block
                    // TODO: Validate transaction.

                    transactions.push(transaction);

                    if transactions.len() >= MAX_TRANSACTIONS_PER_BLOCK {
                        break;
                    }
                }

                let block_number = self.leader_state.block_number + 1;
                let body = Body {
                    leader_term,
                    height: block_number,
                    prev_block_hash: self.leader_state.last_block_hash,
                    transactions,
                };
                let hash = body.hash();

                let transactions = body.transactions;
                log::trace!("Sending block with {} transactions.", transactions.len());

                log::trace!("Sending block with {} transactions.", transactions.len());

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
                let prepare_message = ConsensusMessage::Prepare {
                    leader_term,
                    block_number,
                    block_hash: hash,
                };

                let validate_ackprepares = move |response: &ConsensusMessage| {
                    // This is done for every ACKPREPARE.
                    match response {
                        ConsensusMessage::AckPrepare {
                            leader_term: ack_leader_term,
                            block_number: ack_block_number,
                            block_hash: ack_block_hash,
                        } => {
                            // Check whether the ACKPREPARE is for the same message.
                            if *ack_leader_term == leader_term
                                && *ack_block_number == block_number
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
                let ackprepares = self
                    .broadcast_until_majority(prepare_message, validate_ackprepares)
                    .await;

                let ackprepares = match ackprepares {
                    Ok(ackprepares) => ackprepares,
                    Err(err) => {
                        log::error!(
                            "Consensus error during PREPARE phase for block #{}: {}",
                            block_number,
                            err
                        );
                        // TODO: retry the transactions
                        continue;
                    }
                };
                log::trace!(
                    "Prepare #{} phase ended. Got ACKPREPARE signatures: {:?}",
                    block_number,
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
                    block_number,
                    block_hash: hash,
                    ackprepare_signatures: ackprepares,
                    data: transactions,
                };
                let validate_ackappends = move |response: &ConsensusMessage| {
                    // This is done for every ACKPREPARE.
                    match response {
                        ConsensusMessage::AckAppend {
                            leader_term: ack_leader_term,
                            block_number: ack_block_number,
                            block_hash: ack_block_hash,
                        } => {
                            // Check whether the ACKPREPARE is for the same message.
                            if *ack_leader_term == leader_term
                                && *ack_block_number == block_number
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
                let ackappends = self
                    .broadcast_until_majority(append_message, validate_ackappends)
                    .await;
                let ackappends = match ackappends {
                    Ok(ackappends) => ackappends,
                    Err(err) => {
                        log::error!(
                            "Consensus error during APPEND phase for block #{}: {}",
                            block_number,
                            err
                        );
                        // TODO: retry the transactions
                        continue;
                    }
                };
                log::trace!(
                    "Append Phase #{} ended. Got ACKAPPEND signatures: {:?}",
                    block_number,
                    ackappends,
                );

                // after we collected enough signatures, we can update our state
                self.leader_state.block_number = block_number;
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
                    block_number,
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
                let ackcommits = self
                    .broadcast_until_majority(commit_message, validate_ackcommits)
                    .await;
                match ackcommits {
                    Ok(_) => {
                        log::info!("Comitted block #{} on majority of RPUs.", block_number);
                    }
                    Err(err) => {
                        log::error!(
                            "Consensus error during COMMIT phase for block #{}: {}",
                            block_number,
                            err
                        );
                    }
                }
            }
        }
    }
}
