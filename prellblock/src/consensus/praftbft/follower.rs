use super::{
    super::{Block, BlockHash, BlockNumber, Body, LeaderTerm, SignatureList},
    error::PhaseName,
    message::ConsensusMessage,
    state::{FollowerState, Phase, PhaseMeta, RoundState},
    Error, NewViewSignatures, PRaftBFT,
};
use crate::world_state::WritableWorldState;
use pinxit::{PeerId, Signable, Signed};
use prellblock_client_api::Transaction;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{watch, MutexGuard},
    time,
};

// After this amount of time a transaction should be committed.
const CENSORSHIP_TIMEOUT: Duration = Duration::from_secs(20);

#[allow(clippy::single_match_else)]
impl PRaftBFT {
    /// Wait until we reached the block number the message is at.
    async fn follower_state_in_block(
        &self,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
    ) -> Result<MutexGuard<'_, FollowerState>, Error> {
        let follower_state = self
            .synchronize_if_needed(leader_term, block_number)
            .await?;

        if follower_state.block_number + 1 >= block_number {
            return Ok(follower_state);
        }
        drop(follower_state);

        let mut receiver = self.block_changed_receiver.clone();
        loop {
            // Wait until block number changed.
            let _ = receiver.recv().await;
            let follower_state = self.follower_state.lock().await;
            if follower_state.block_number + 1 >= block_number {
                return Ok(follower_state);
            }
        }
    }

    async fn handle_prepare_message(
        &self,
        peer_id: &PeerId,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
        block_hash: BlockHash,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self
            .follower_state_in_block(leader_term, block_number)
            .await?;

        log::trace!("Handle Prepare message #{}.", block_number);
        self.verify_message_meta(&follower_state, peer_id, leader_term, block_number)?;

        // Check whether the state for the block is Waiting.
        // We only allow to receive messages once.
        let round_state = follower_state.round_state(block_number)?;
        if !matches!(round_state.phase, Phase::Waiting) {
            return Err(Error::WrongPhase {
                current: round_state.phase.to_phase_name(),
                expected: PhaseName::Waiting,
            });
        }

        // All checks passed, update our state.
        let leader = self.leader(follower_state.leader_term);
        follower_state.round_state_mut(block_number).unwrap().phase =
            Phase::Prepare(PhaseMeta { leader, block_hash });

        // Send AckPrepare to the leader.
        // *Note*: Technically, we only need to send a signature of
        // the PREPARE message.
        let ackprepare_message = ConsensusMessage::AckPrepare {
            leader_term: follower_state.leader_term,
            block_number,
            block_hash,
        };

        // Done :D
        Ok(ackprepare_message)
    }

    pub(super) async fn rollback_last_block(&self, follower_state: &mut FollowerState) {
        log::trace!("Doing rollback.");

        // better save than sorry
        follower_state.rollback_possible = false;

        // Rollback WorldState by one block.
        log::trace!(
            "Rollback: Last block hash before rollback: {}.",
            self.world_state.get().last_block_hash
        );
        self.world_state.rollback().unwrap();
        log::trace!(
            "Rollback: Last block hash after rollback: {}.",
            self.world_state.get().last_block_hash,
        );

        // BlockStorage remove topmost block.
        // Double Unwrap should be fine because there needs to be some block.
        let removed_block = self.block_storage.pop_block().unwrap().unwrap();
        let removed_block_number = removed_block.block_number();

        // The transactions may not be lost.
        self.take_transactions(removed_block.body.transactions)
            .await;

        // Reset RoundState (decrement)
        follower_state.round_states.decrement(RoundState {
            phase: Phase::Committed(removed_block.body.prev_block_hash),
            buffered_commit_message: None,
        });
        *follower_state
            .round_state_mut(removed_block_number)
            .unwrap() = RoundState {
            phase: Phase::Waiting,
            buffered_commit_message: None,
        };

        // FollowerState reset / from WorldState
        follower_state.block_number -= 1;
        log::trace!("Done rollback.");
    }

    #[allow(clippy::too_many_lines)]
    async fn handle_append_message(
        &self,
        peer_id: &PeerId,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
        block_hash: BlockHash,
        ackprepare_signatures: SignatureList,
        data: Vec<Signed<Transaction>>,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state.lock().await;
        let mut follower_state = if follower_state.rollback_possible
            && block_number == follower_state.block_number
        {
            // Check validity of signatures.
            // FIXME: Code duplication.
            let ackprepare_message = ConsensusMessage::AckPrepare {
                leader_term,
                block_number,
                block_hash,
            };
            match self.verify_rpu_majority_signatures(&ackprepare_message, &ackprepare_signatures) {
                Ok(()) => {}
                Err(err) => {
                    self.request_view_change(follower_state).await;
                    return Err(err);
                }
            }
            self.rollback_last_block(&mut follower_state).await;
            follower_state
        } else {
            drop(follower_state);
            let follower_state = self
                .follower_state_in_block(leader_term, block_number)
                .await?;

            log::trace!("Handle Append message #{}.", block_number);
            self.verify_message_meta(&follower_state, peer_id, leader_term, block_number)?;
            follower_state
        };

        // Check whether the state for the block is Prepare.
        // We only allow to receive messages once.
        let round_state = follower_state.round_state(block_number)?;
        let meta = match &round_state.phase {
            Phase::Prepare(meta) => meta.clone(),
            Phase::Waiting => {
                let leader = self.leader(follower_state.leader_term);
                PhaseMeta { leader, block_hash }
            }
            _ => {
                return Err(Error::WrongPhase {
                    current: round_state.phase.to_phase_name(),
                    expected: PhaseName::Prepare,
                });
            }
        };

        // Check validity of ACKPREPARE Signatures.
        let ackprepare_message = ConsensusMessage::AckPrepare {
            leader_term,
            block_number,
            block_hash,
        };
        follower_state = self
            .check_block_and_verify_signatures(
                follower_state,
                &meta,
                block_hash,
                block_number,
                &ackprepare_message,
                &ackprepare_signatures,
            )
            .await?;

        if data.is_empty() {
            // Empty blocks are not allowed.
            // Trigger leader change as a consequence.
            log::error!("Received empty block.");
            self.request_view_change(follower_state).await;
            return Err(Error::EmptyBlock);
        }

        // Check for transaction validity.
        match self.transaction_checker.verify_signatures(&data) {
            Ok(()) => {}
            Err(err) => {
                log::error!("Error while verifying transaction signature: {}", err);
                self.request_view_change(follower_state).await;
                return Err(err.into());
            }
        }

        // TODO: Remove if we completely deny this block.
        let validated_transactions = data;

        // Validate the Block Hash.
        let body = Body {
            leader_term,
            height: block_number,
            prev_block_hash: follower_state.last_block_hash(),
            transactions: validated_transactions,
        };
        if block_hash != body.hash() {
            return Err(Error::BlockNotMatchingHash);
        }
        let leader_term = follower_state.leader_term;

        // All checks passed, update our state.
        let round_state_mut = follower_state.round_state_mut(block_number).unwrap();
        round_state_mut.phase = Phase::Append(meta, body);

        // There could be a commit message for this block number that arrived first.
        // We then need to apply the commit (or at least check).
        if let Some(buffered_message) = round_state_mut.buffered_commit_message.take() {
            match buffered_message {
                ConsensusMessage::Commit {
                    leader_term: buffered_leader_term,
                    block_number: buffered_block_number,
                    block_hash: buffered_block_hash,
                    ackappend_signatures: buffered_ackappend_signatures,
                } => {
                    let commit_result = self
                        .handle_commit_message_inner(
                            follower_state,
                            peer_id,
                            buffered_leader_term,
                            buffered_block_number,
                            buffered_block_hash,
                            buffered_ackappend_signatures,
                        )
                        .await;
                    match commit_result {
                        Ok(_) => log::debug!("Used out-of-order commit."),
                        Err(err) => log::debug!("Failed to apply out-of-order commit: {}", err),
                    }
                }
                _ => unreachable!(),
            }
        }

        let ackappend_message = ConsensusMessage::AckAppend {
            leader_term,
            block_number,
            block_hash,
        };
        Ok(ackappend_message)
    }

    async fn handle_commit_message(
        &self,
        peer_id: &PeerId,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
        block_hash: BlockHash,
        ackappend_signatures: SignatureList,
    ) -> Result<ConsensusMessage, Error> {
        let follower_state = self
            .follower_state_in_block(leader_term, block_number)
            .await?;
        self.handle_commit_message_inner(
            follower_state,
            peer_id,
            leader_term,
            block_number,
            block_hash,
            ackappend_signatures,
        )
        .await
    }

    /// This function is used for out-of-order message reception and
    /// applying these commits.
    async fn handle_commit_message_inner(
        &self,
        mut follower_state: MutexGuard<'_, FollowerState>,
        peer_id: &PeerId,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
        block_hash: BlockHash,
        ackappend_signatures: SignatureList,
    ) -> Result<ConsensusMessage, Error> {
        log::trace!("Handle Commit message #{}.", block_number);
        self.verify_message_meta(&follower_state, peer_id, leader_term, block_number)?;

        // Check whether the state for the block is Append.
        // We only allow to receive messages once.
        let round_state = follower_state.round_state(block_number)?;
        let (meta, body) = match &round_state.phase {
            Phase::Waiting | Phase::Prepare(..) => {
                let current_phase_name = round_state.phase.to_phase_name();
                let consensus_message = ConsensusMessage::Commit {
                    leader_term,
                    block_number,
                    block_hash,
                    ackappend_signatures,
                };
                follower_state
                    .round_state_mut(block_number)
                    .unwrap()
                    .buffered_commit_message = Some(consensus_message);
                return Err(Error::WrongPhase {
                    current: current_phase_name,
                    expected: PhaseName::Append,
                });
            }
            Phase::Append(meta, body) => (meta.clone(), body.clone()),
            _ => {
                return Err(Error::WrongPhase {
                    current: round_state.phase.to_phase_name(),
                    expected: PhaseName::Append,
                });
            }
        };

        // Check validity of ACKAPPEND Signatures.
        let ackappend_message = ConsensusMessage::AckAppend {
            leader_term,
            block_number,
            block_hash,
        };
        let mut follower_state = self
            .check_block_and_verify_signatures(
                follower_state,
                &meta,
                block_hash,
                block_number,
                &ackappend_message,
                &ackappend_signatures,
            )
            .await?;

        let block = Block {
            body,
            signatures: ackappend_signatures,
        };
        let number_of_transactions = block.body.transactions.len();

        // Write Block to WorldState
        let world_state = self.world_state.get_writable().await;
        self.increment_state_and_write_block(&mut follower_state, world_state, block, block_hash)
            .await;

        log::debug!(
            "Committed block #{} with hash {:?} and {} transactions.",
            block_number,
            block_hash,
            number_of_transactions,
        );
        Ok(ConsensusMessage::AckCommit)
    }

    /// Process the incoming `ConsensusMessages` (`PREPARE`, `ACKPREPARE`, `APPEND`, `ACKAPPEND`, `COMMIT`).
    pub async fn handle_message(
        self: &Arc<Self>,
        message: Signed<ConsensusMessage>,
    ) -> Result<Signed<ConsensusMessage>, Error> {
        // Only RPUs are allowed.
        if !self.peer_ids().any(|peer_id| *message.signer() == peer_id) {
            return Err(Error::InvalidPeer(message.signer().clone()));
        }

        let message = message.verify()?;
        let peer_id = message.signer().clone();
        let signature = message.signature().clone();

        let response = match message.into_inner() {
            ConsensusMessage::Prepare {
                leader_term,
                block_number,
                block_hash,
            } => {
                self.handle_prepare_message(&peer_id, leader_term, block_number, block_hash)
                    .await?
            }
            ConsensusMessage::Append {
                leader_term,
                block_number,
                block_hash,
                ackprepare_signatures,
                data,
            } => {
                self.handle_append_message(
                    &peer_id,
                    leader_term,
                    block_number,
                    block_hash,
                    ackprepare_signatures,
                    data,
                )
                .await?
            }
            ConsensusMessage::Commit {
                leader_term,
                block_number,
                block_hash,
                ackappend_signatures,
            } => {
                self.handle_commit_message(
                    &peer_id,
                    leader_term,
                    block_number,
                    block_hash,
                    ackappend_signatures,
                )
                .await?
            }
            ConsensusMessage::ViewChange { new_leader_term } => {
                self.handle_view_change(peer_id, signature, new_leader_term)
                    .await?
            }
            ConsensusMessage::NewView {
                leader_term,
                view_change_signatures,
                current_block_number,
            } => {
                self.handle_new_view(
                    &peer_id,
                    leader_term,
                    view_change_signatures,
                    current_block_number,
                )
                .await?
            }
            ConsensusMessage::SynchronizationRequest {
                leader_term,
                block_number,
                block_hash,
            } => {
                self.handle_synchronization_request(&peer_id, leader_term, block_number, block_hash)
                    .await?
            }
            ConsensusMessage::AckPrepare { .. }
            | ConsensusMessage::AckAppend { .. }
            | ConsensusMessage::AckCommit { .. }
            | ConsensusMessage::AckViewChange { .. }
            | ConsensusMessage::AckNewView { .. }
            | ConsensusMessage::SynchronizationResponse { .. } => {
                return Err(Error::UnexpectedMessage);
            }
        };

        let signed_response = response.sign(&self.identity).unwrap();
        Ok(signed_response)
    }

    /// This is woken up after a timeout or a specific
    /// number of blocks commited.
    pub(super) async fn censorship_checker(
        &self,
        mut new_view_receiver: watch::Receiver<(LeaderTerm, NewViewSignatures)>,
    ) {
        loop {
            let timeout_result = time::timeout(CENSORSHIP_TIMEOUT, new_view_receiver.recv()).await;
            // If there was no timeout, a leader change happened.
            // Give the leader enough time by sleeping again.
            if timeout_result.is_ok() {
                continue;
            }

            let queue = self.queue.read().await;
            // Iterating over the queue should be pretty fast.
            // If there are no old transactions, we should only have
            // a few transactions to iterate over.
            let has_old_transactions = queue
                .iter()
                .any(|(timestamp, _)| timestamp.elapsed() > CENSORSHIP_TIMEOUT);
            drop(queue);

            if has_old_transactions {
                // leader seems to be faulty / dead or censoring
                let follower_state = self.follower_state.lock().await;
                let leader = self.leader(follower_state.leader_term);
                log::warn!(
                    "Found censored transactions from leader {}. Requesting View Change.",
                    leader
                );
                self.request_view_change(follower_state).await;
            } else {
                log::trace!("No old transactions found while checking for censorship.");
            }
        }
    }

    /// Validate that the *received* message is from
    /// the current leader (`leader_term`) and has a valid `block_number`.
    fn verify_message_meta(
        &self,
        follower_state: &FollowerState,
        peer_id: &PeerId,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
    ) -> Result<(), Error> {
        if !self.is_current_leader(leader_term, peer_id) {
            log::warn!("Received message from invalid leader (ID: {}).", peer_id);
            return Err(Error::WrongLeader(peer_id.clone()));
        }

        // We only handle the current leader term.
        if leader_term != follower_state.leader_term {
            let error = Error::WrongLeaderTerm;
            log::warn!("{}", error);
            return Err(error);
        }

        // Only process new messages.
        if block_number <= follower_state.block_number {
            let error = Error::BlockNumberTooSmall(block_number);
            log::warn!("{}", error);
            return Err(error);
        }

        Ok(())
    }

    async fn check_block_and_verify_signatures<'a>(
        &'a self,
        follower_state: MutexGuard<'a, FollowerState>,
        meta: &PhaseMeta,
        block_hash: BlockHash,
        block_number: BlockNumber,
        message: &ConsensusMessage,
        signatures: &SignatureList,
    ) -> Result<MutexGuard<'a, FollowerState>, Error> {
        if block_hash != meta.block_hash {
            return Err(Error::ChangedBlockHash);
        }

        let expected_block_number = follower_state.block_number + 1;
        if block_number != expected_block_number {
            return Err(Error::PrevBlockNumberDoesNotMatch(
                block_number,
                expected_block_number,
            ));
        }

        // Check validity of signatures.
        match self.verify_rpu_majority_signatures(message, signatures) {
            Ok(()) => {}
            Err(err) => {
                self.request_view_change(follower_state).await;
                return Err(err);
            }
        }

        Ok(follower_state)
    }

    pub(super) async fn increment_state_and_write_block(
        &self,
        follower_state: &mut FollowerState,
        mut world_state: WritableWorldState,
        block: Block,
        block_hash: BlockHash,
    ) {
        {
            let round_state = follower_state
                .round_state_mut(block.block_number())
                .unwrap();
            round_state.buffered_commit_message = None;
            round_state.phase = Phase::Committed(block_hash);
        }

        let old_round_state = follower_state.round_states.increment(RoundState::default());
        assert!(matches!(old_round_state.phase, Phase::Committed(..)));
        assert!(old_round_state.buffered_commit_message.is_none());

        follower_state.block_number = block.block_number();
        // No rollback possible after one commit.
        follower_state.rollback_possible = false;

        let _ = self.block_changed_notifier.broadcast(());

        // Write Block to BlockStorage
        self.block_storage.write_block(&block).unwrap();

        // Remove committed transactions from our queue.
        self.queue
            .write()
            .await
            .retain(|(_, transaction)| !block.body.transactions.contains(transaction));

        // Apply to the WorldState.
        world_state.apply_block(block).unwrap();
        world_state.save();
    }
}
