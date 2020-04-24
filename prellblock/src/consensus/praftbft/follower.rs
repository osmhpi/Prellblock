use super::{
    super::{Block, BlockHash, Body},
    error::PhaseName,
    message::ConsensusMessage,
    state::{Phase, PhaseMeta},
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
        let phase = follower_state.phase(sequence_number)?;
        if !matches!(phase, Phase::Waiting) {
            return Err(Error::WrongPhase {
                received: phase.to_phase_name(),
                expected: PhaseName::Waiting,
            });
        }

        // All checks passed, update our state.
        let leader = follower_state.leader.clone().unwrap();
        follower_state
            .set_phase(
                sequence_number,
                Phase::Prepare(PhaseMeta { leader, block_hash }),
            )
            .unwrap();

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
        let phase = follower_state.phase(sequence_number)?;
        let meta = match phase {
            Phase::Prepare(meta) => meta,
            _ => {
                return Err(Error::WrongPhase {
                    received: phase.to_phase_name(),
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
        let meta = meta.clone();
        follower_state
            .set_phase(sequence_number, Phase::Append(meta, body))
            .unwrap();

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
        let phase = follower_state.phase(sequence_number)?;
        let (meta, body) = match phase {
            Phase::Append(meta, body) => (meta, body.clone()),
            _ => {
                return Err(Error::WrongPhase {
                    received: phase.to_phase_name(),
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
        for (peer_id, signature) in &ackappend_signatures {
            // Frage: Was tun bei faulty signature? Abbrechen oder weiter bei Supermajority?
            peer_id.verify(&ackprepare_message, signature)?;
        }

        follower_state
            .set_phase(sequence_number, Phase::Committed(block_hash))
            .unwrap();
        match follower_state.round_state.increment(Phase::Waiting) {
            Phase::Committed(..) => {}
            _ => unreachable!(),
        }
        follower_state.sequence = sequence_number;

        // Write Block to BlockStorage
        self.block_storage
            .write_block(&Block {
                body,
                signatures: ackappend_signatures,
            })
            .unwrap();
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
