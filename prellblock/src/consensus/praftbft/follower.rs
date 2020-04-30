use super::{
    super::{Block, BlockHash, Body, SequenceNumber},
    error::PhaseName,
    message::ConsensusMessage,
    state::{FollowerState, Phase, PhaseMeta, RoundState},
    Error, PRaftBFT,
};
use pinxit::{PeerId, Signable, Signature, Signed};
use prellblock_client_api::Transaction;
use std::time::Duration;
use tokio::{
    sync::{watch, MutexGuard},
    time,
};

// After this amount of time a transaction should be committed.
const CENSORSHIP_TIMEOUT: Duration = Duration::from_secs(10);

#[allow(clippy::single_match_else)]
impl PRaftBFT {
    /// Wait until we reached the sequence number the message is at.
    async fn follower_state_in_sequence(
        &self,
        sequence_number: SequenceNumber,
    ) -> MutexGuard<'_, FollowerState> {
        let mut receiver = self.sequence_changed_receiver.clone();
        loop {
            let follower_state = self.follower_state.lock().await;
            if follower_state.sequence + 1 >= sequence_number {
                return follower_state;
            }
            drop(follower_state);
            // Wait until sequence number changed.
            let _ = receiver.recv().await;
        }
    }

    async fn handle_prepare_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: SequenceNumber,
        block_hash: BlockHash,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state_in_sequence(sequence_number).await;
        if !self.is_current_leader(leader_term, peer_id) {
            log::warn!("Received message from invalid leader (ID: {}).", peer_id);
            return Err(Error::WrongLeader(peer_id.clone()));
        }
        follower_state.verify_message_meta(leader_term, sequence_number)?;

        // Check whether the state for the sequence is Waiting.
        // We only allow to receive messages once.
        let round_state = follower_state.round_state(sequence_number)?;
        if !matches!(round_state.phase, Phase::Waiting) {
            return Err(Error::WrongPhase {
                current: round_state.phase.to_phase_name(),
                expected: PhaseName::Waiting,
            });
        }

        // All checks passed, update our state.
        let leader = self.leader(follower_state.leader_term);
        follower_state
            .round_state_mut(sequence_number)
            .unwrap()
            .phase = Phase::Prepare(PhaseMeta { leader, block_hash });

        // Send AckPrepare to the leader.
        // *Note*: Technically, we only need to send a signature of
        // the PREPARE message.
        let ackprepare_message = ConsensusMessage::AckPrepare {
            leader_term: follower_state.leader_term,
            sequence_number,
            block_hash,
        };

        // Done :D
        Ok(ackprepare_message)
    }

    async fn handle_append_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: SequenceNumber,
        block_hash: BlockHash,
        ackprepare_signatures: Vec<(PeerId, Signature)>,
        data: Vec<Signed<Transaction>>,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state_in_sequence(sequence_number).await;
        log::trace!("Handle Append message #{}.", sequence_number);
        if !self.is_current_leader(leader_term, peer_id) {
            log::warn!("Received message from invalid leader (ID: {}).", peer_id);
            return Err(Error::WrongLeader(peer_id.clone()));
        }
        follower_state.verify_message_meta(leader_term, sequence_number)?;

        // Check whether the state for the sequence is Prepare.
        // We only allow to receive messages once.
        let round_state = follower_state.round_state(sequence_number)?;
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

        if block_hash != meta.block_hash {
            return Err(Error::ChangedBlockHash);
        }

        if sequence_number != follower_state.sequence + 1 {
            return Err(Error::WrongSequenceNumber(sequence_number));
        }

        // Check validity of ACKPREPARE Signatures.
        if !self.supermajority_reached(ackprepare_signatures.len()) {
            return Err(Error::NotEnoughSignatures);
        }

        let ackprepare_message = ConsensusMessage::AckPrepare {
            leader_term,
            sequence_number,
            block_hash,
        };
        for (peer_id, signature) in ackprepare_signatures {
            // Frage: Was tun bei faulty signature? Abbrechen oder weiter bei Supermajority?
            peer_id.verify(&ackprepare_message, &signature)?;
        }

        // Check for transaction validity.
        for tx in data.clone() {
            tx.verify()?;
        }

        // TODO: Stateful validate transactions HERE.
        let validated_transactions = data;

        // Validate the Block Hash.
        let body = Body {
            height: sequence_number,
            prev_block_hash: follower_state.last_block_hash(),
            transactions: validated_transactions,
        };
        if block_hash != body.hash() {
            return Err(Error::WrongBlockHash);
        }

        // All checks passed, update our state.
        let round_state_mut = follower_state.round_state_mut(sequence_number).unwrap();
        round_state_mut.phase = Phase::Append(meta, body);

        // There could be a commit message for this sequence number that arrived first.
        // We then need to apply the commit (or at least check).
        if let Some(buffered_message) = round_state_mut.buffered_commit_message.take() {
            match buffered_message {
                ConsensusMessage::Commit {
                    leader_term: buffered_leader_term,
                    sequence_number: buffered_sequence_number,
                    block_hash: buffered_block_hash,
                    ackappend_signatures: buffered_ackappend_signatures,
                } => {
                    let commit_result = self
                        .handle_commit_message_inner(
                            &mut follower_state,
                            peer_id,
                            buffered_leader_term,
                            buffered_sequence_number,
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
            leader_term: follower_state.leader_term,
            sequence_number,
            block_hash,
        };
        Ok(ackappend_message)
    }

    async fn handle_commit_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: SequenceNumber,
        block_hash: BlockHash,
        ackappend_signatures: Vec<(PeerId, Signature)>,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state_in_sequence(sequence_number).await;
        self.handle_commit_message_inner(
            &mut follower_state,
            peer_id,
            leader_term,
            sequence_number,
            block_hash,
            ackappend_signatures,
        )
        .await

        // +--------------------------------------------+
        // | TODO: Use this when view change is needed. |
        // +--------------------------------------------+
        // let new_leader_term = follower_state.leader_term + 1;
        // let messages = HashMap::new();
        // follower_state.set_view_phase(
        //     new_leader_term,
        //     ViewPhase::ViewChanging(ViewPhaseMeta { messages }),
        // )?;
        // drop(follower_state);
        // self.broadcast_view_change(new_leader_term).await.unwrap();
    }

    /// This function is used for out-of-order message reception and
    /// applying these commits.
    async fn handle_commit_message_inner(
        &self,
        follower_state: &mut FollowerState,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: SequenceNumber,
        block_hash: BlockHash,
        ackappend_signatures: Vec<(PeerId, Signature)>,
    ) -> Result<ConsensusMessage, Error> {
        log::trace!("Handle Commit message #{}.", sequence_number);
        if !self.is_current_leader(leader_term, peer_id) {
            log::warn!("Received message from invalid leader (ID: {}).", peer_id);
            return Err(Error::WrongLeader(peer_id.clone()));
        }
        follower_state.verify_message_meta(leader_term, sequence_number)?;

        // Check whether the state for the sequence is Append.
        // We only allow to receive messages once.
        let round_state = follower_state.round_state(sequence_number)?;
        let (meta, body) = match &round_state.phase {
            Phase::Waiting | Phase::Prepare(..) => {
                let current_phase_name = round_state.phase.to_phase_name();
                let consensus_message = ConsensusMessage::Commit {
                    leader_term,
                    sequence_number,
                    block_hash,
                    ackappend_signatures,
                };
                follower_state
                    .round_state_mut(sequence_number)
                    .unwrap()
                    .buffered_commit_message = Some(consensus_message);
                return Err(Error::WrongPhase {
                    current: current_phase_name,
                    expected: PhaseName::Append,
                });
            }
            Phase::Append(meta, body) => (meta, body.clone()),
            _ => {
                return Err(Error::WrongPhase {
                    current: round_state.phase.to_phase_name(),
                    expected: PhaseName::Append,
                });
            }
        };

        if block_hash != meta.block_hash {
            return Err(Error::ChangedBlockHash);
        }

        if sequence_number != follower_state.sequence + 1 {
            return Err(Error::WrongSequenceNumber(sequence_number));
        }

        // Check validity of ACKAPPEND Signatures.
        if !self.supermajority_reached(ackappend_signatures.len()) {
            return Err(Error::NotEnoughSignatures);
        }
        let ackprepare_message = ConsensusMessage::AckAppend {
            leader_term,
            sequence_number,
            block_hash,
        };
        for (peer_id, signature) in &ackappend_signatures {
            // Frage: Was tun bei faulty signature? Abbrechen oder weiter bei Supermajority?
            peer_id.verify(&ackprepare_message, signature)?;
        }

        follower_state
            .round_state_mut(sequence_number)
            .unwrap()
            .phase = Phase::Committed(block_hash);

        let old_round_state = follower_state.round_states.increment(RoundState::default());
        assert!(matches!(old_round_state.phase, Phase::Committed(..)));
        assert!(old_round_state.buffered_commit_message.is_none());

        follower_state.sequence = sequence_number;
        let _ = self.sequence_changed_notifier.broadcast(());

        let block = Block {
            body,
            signatures: ackappend_signatures,
        };
        // Write Block to BlockStorage
        self.block_storage.write_block(&block).unwrap();

        // Remove committed transactions from our queue.
        self.queue
            .write()
            .await
            .retain(|(_, transaction)| !block.body.transactions.contains(transaction));

        // Write Block to WorldState
        let mut world_state = self.world_state.get_writable().await;
        world_state.apply_block(block).unwrap();
        world_state.save();

        log::debug!(
            "Committed block #{} with hash {:?}.",
            sequence_number,
            block_hash
        );
        Ok(ConsensusMessage::AckCommit)
    }

    /// Process the incoming `ConsensusMessages` (`PREPARE`, `ACKPREPARE`, `APPEND`, `ACKAPPEND`, `COMMIT`).
    pub async fn handle_message(
        &self,
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
                sequence_number,
                block_hash,
            } => {
                self.handle_prepare_message(&peer_id, leader_term, sequence_number, block_hash)
                    .await?
            }
            ConsensusMessage::Append {
                leader_term,
                sequence_number,
                block_hash,
                ackprepare_signatures,
                data,
            } => {
                self.handle_append_message(
                    &peer_id,
                    leader_term,
                    sequence_number,
                    block_hash,
                    ackprepare_signatures,
                    data,
                )
                .await?
            }
            ConsensusMessage::Commit {
                leader_term,
                sequence_number,
                block_hash,
                ackappend_signatures,
            } => {
                self.handle_commit_message(
                    &peer_id,
                    leader_term,
                    sequence_number,
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
            } => {
                self.handle_new_view(&peer_id, leader_term, view_change_signatures)
                    .await?
            }
            _ => unimplemented!(),
        };

        let signed_response = response.sign(&self.broadcast_meta.identity).unwrap();
        Ok(signed_response)
    }

    /// This is woken up after a timeout or a specific
    /// number of blocks commited.
    pub(super) async fn censorship_checker(&self, mut new_view_receiver: watch::Receiver<usize>) {
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

            // do stomething if there where old transactions
            if has_old_transactions {
                let leader = self.leader(self.follower_state.lock().await.leader_term);
                log::warn!("Found censored transactions from leader {}.", leader);
            }
        }
    }
}
