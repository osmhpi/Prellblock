use super::{
    super::{BlockHash, Body},
    error::PhaseName,
    message::ConsensusMessage,
    state::{Phase, PhaseMeta, RoundState},
    Error, PRaftBFT,
};
use pinxit::{PeerId, Signable, Signature, Signed};
use prellblock_client_api::Transaction;

#[allow(clippy::single_match_else)]
impl PRaftBFT {
    fn handle_prepare_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: u64,
        block_hash: BlockHash,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state.lock().unwrap();
        follower_state.verify_message_meta(peer_id, leader_term, sequence_number)?;

        // Check whether the state for the sequence is Waiting.
        // We only allow to receive messages once.
        let round_state = follower_state.round_state(sequence_number)?;
        if !matches!(round_state.phase, Phase::Waiting) {
            return Err(Error::WrongPhase {
                received: round_state.phase.to_phase_name(),
                expected: PhaseName::Waiting,
            });
        }

        // All checks passed, update our state.
        let leader = follower_state.leader.clone().unwrap();
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

    fn handle_append_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: u64,
        block_hash: BlockHash,
        ackprepare_signatures: Vec<(PeerId, Signature)>,
        data: Vec<Signed<Transaction>>,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state.lock().unwrap();
        follower_state.verify_message_meta(peer_id, leader_term, sequence_number)?;

        // Check whether the state for the sequence is Prepare.
        // We only allow to receive messages once.
        let round_state = follower_state.round_state(sequence_number)?;
        let meta = match &round_state.phase {
            Phase::Prepare(meta) => meta.clone(),
            Phase::Waiting => {
                let leader = follower_state.leader.clone().unwrap();
                PhaseMeta { leader, block_hash }
            }
            _ => {
                return Err(Error::WrongPhase {
                    received: round_state.phase.to_phase_name(),
                    expected: PhaseName::Prepare,
                });
            }
        };

        if block_hash != meta.block_hash {
            return Err(Error::ChangedBlockHash);
        }

        if sequence_number != follower_state.sequence + 1 {
            return Err(Error::WrongSequenceNumber);
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
        let round_state = follower_state.round_state_mut(sequence_number).unwrap();
        round_state.phase = Phase::Append(meta, body);

        // There could be a commit message for this sequence number that arrived first.
        // We then need to apply the commit (or at least check).
        if let Some(buffered_message) = round_state.buffered_commit_message.take() {
            match buffered_message {
                ConsensusMessage::Commit {
                    leader_term,
                    sequence_number,
                    block_hash,
                    ackappend_signatures,
                } => {
                    let commit_result = self.handle_commit_message(
                        peer_id,
                        leader_term,
                        sequence_number,
                        block_hash,
                        ackappend_signatures,
                    );
                    match commit_result {
                        Ok(_) => log::trace!("Used out-of-order commit."),
                        Err(err) => log::trace!("Failed to apply out-of-order commit: {}", err),
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

    fn handle_commit_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: u64,
        block_hash: BlockHash,
        ackappend_signatures: Vec<(PeerId, Signature)>,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state.lock().unwrap();
        follower_state.verify_message_meta(peer_id, leader_term, sequence_number)?;

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
                    received: current_phase_name,
                    expected: PhaseName::Append,
                });
            }
            Phase::Append(meta, body) => (meta, body),
            _ => {
                return Err(Error::WrongPhase {
                    received: round_state.phase.to_phase_name(),
                    expected: PhaseName::Append,
                });
            }
        };

        if block_hash != meta.block_hash {
            return Err(Error::ChangedBlockHash);
        }

        if sequence_number != follower_state.sequence + 1 {
            return Err(Error::WrongSequenceNumber);
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
        for (peer_id, signature) in ackappend_signatures {
            // Frage: Was tun bei faulty signature? Abbrechen oder weiter bei Supermajority?
            peer_id.verify(&ackprepare_message, &signature)?;
        }

        follower_state
            .round_state_mut(sequence_number)
            .unwrap()
            .phase = Phase::Committed(block_hash);

        let old_round_state = follower_state.round_states.increment(RoundState::default());
        assert!(matches!(old_round_state.phase, Phase::Committed(..)));
        assert!(old_round_state.buffered_commit_message.is_none());

        follower_state.sequence = sequence_number;

        // Write Blocks to BlockStorage
        let _ = body;
        log::debug!(
            "Committed block #{} with hash {:?}.",
            sequence_number,
            block_hash
        );

        Ok(ConsensusMessage::AckCommit)
    }

    /// Process the incoming `ConsensusMessages` (`PREPARE`, `ACKPREPARE`, `APPEND`, `ACKAPPEND`, `COMMIT`).
    pub fn handle_message(
        &self,
        message: Signed<ConsensusMessage>,
    ) -> Result<Signed<ConsensusMessage>, Error> {
        // Only RPUs are allowed.
        if !self.peers.contains_key(message.signer()) {
            return Err(Error::InvalidPeer(message.signer().clone()));
        }

        let message = message.verify()?;
        let peer_id = message.signer().clone();

        let response = match message.into_inner() {
            ConsensusMessage::Prepare {
                leader_term,
                sequence_number,
                block_hash,
            } => self.handle_prepare_message(&peer_id, leader_term, sequence_number, block_hash)?,
            ConsensusMessage::Append {
                leader_term,
                sequence_number,
                block_hash,
                ackprepare_signatures,
                data,
            } => self.handle_append_message(
                &peer_id,
                leader_term,
                sequence_number,
                block_hash,
                ackprepare_signatures,
                data,
            )?,
            ConsensusMessage::Commit {
                leader_term,
                sequence_number,
                block_hash,
                ackappend_signatures,
            } => self.handle_commit_message(
                &peer_id,
                leader_term,
                sequence_number,
                block_hash,
                ackappend_signatures,
            )?,
            _ => unimplemented!(),
        };

        let signed_response = response.sign(&self.identity).unwrap();
        Ok(signed_response)
    }
}
