use super::{super::BlockHash, message::ConsensusMessage, Error, PRaftBFT};
use pinxit::{PeerId, Signable, Signature, Signed};
use prellblock_client_api::Transaction;

impl PRaftBFT {
    fn handle_prepare_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: usize,
        block_hash: BlockHash,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state.lock().unwrap();
        follower_state.verify_message_meta(peer_id, leader_term, sequence_number)?;

        // All checks passed, update our state.
        follower_state.current_block_hash = block_hash;

        // Send AckPrepare to the leader.
        // *Note*: Technically, we only need to send a signature of
        // the PREPARE message.
        let ackprepare_message = ConsensusMessage::AckPrepare {
            leader_term: follower_state.leader_term,
            sequence_number,
            block_hash: follower_state.current_block_hash,
        };

        // Done :D
        Ok(ackprepare_message)
    }

    fn handle_append_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: usize,
        block_hash: BlockHash,
        ackprepare_signatures: Vec<(PeerId, Signature)>,
        data: Vec<Signed<Transaction>>,
    ) -> Result<ConsensusMessage, Error> {
        let follower_state = self.follower_state.lock().unwrap();
        follower_state.verify_message_meta(peer_id, leader_term, sequence_number)?;

        // Check for transaction validity
        // Check for Correctnes of Ackprepare Signatures

        // TODO: Remove this.
        let _ = (block_hash, ackprepare_signatures, data);
        unimplemented!();
    }

    /// Process the incoming `ConsensusMessages` (`PREPARE`, `ACKPREPARE`, `APPEND`, `ACKAPPEND`, `COMMIT`)
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
            _ => unimplemented!(),
        };

        let signed_response = response.sign(&self.identity).unwrap();
        Ok(signed_response)
    }
}
