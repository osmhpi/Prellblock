use super::{
    super::{BlockNumber, LeaderTerm, SignatureList},
    message::ConsensusMessage,
    state::FollowerState,
    Error, PRaftBFT,
};
use crate::peer::{message as peer_message, Sender};
use pinxit::{PeerId, Signable};
use rand::Rng;
use tokio::sync::MutexGuard;

#[allow(clippy::single_match_else)]
impl PRaftBFT {
    pub(super) fn do_we_need_to_synchronize(
        &self,
        follower_state: &FollowerState,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
    ) -> bool {
        let _ = self;
        leader_term > follower_state.leader_term || block_number > follower_state.block_number + 1
    }

    pub(super) async fn synchronize(
        &self,
        follower_state: MutexGuard<'_, FollowerState>,
    ) -> Result<MutexGuard<'_, FollowerState>, Error> {
        self.synchronize_inner(follower_state).await.map_err(|err| {
            log::error!("Synchronization error: {}", err);
            err
        })
    }

    async fn synchronize_inner(
        &self,
        follower_state: MutexGuard<'_, FollowerState>,
    ) -> Result<MutexGuard<'_, FollowerState>, Error> {
        let leader_term = follower_state.leader_term;
        let block_number = follower_state.block_number;
        drop(follower_state);

        // TODO: implement stuff
        // choose peer to ask for synchronization
        let peers = self.peers();
        let peer_index = rand::thread_rng().gen_range(0, peers.len());
        let peer = &peers[peer_index];

        // send request to peer
        let mut sender = Sender::new(peer.1);

        // verify received blocks, if not timeouted
        // append correct blocks to own blockstorage
        let consensus_message = ConsensusMessage::SynchronizationRequest {
            leader_term,
            block_number,
        };
        let signed_consensus_message = consensus_message.sign(&self.identity)?;
        let request = peer_message::Consensus(signed_consensus_message);
        let response = sender.send_request(request).await?;

        let (new_view, blocks) = match response.verify()?.into_inner() {
            ConsensusMessage::SynchronizationResponse { new_view, blocks } => (new_view, blocks),
            _ => return Err(Error::UnexpectedResponse),
        };

        let mut follower_state = self.follower_state.lock().await;
        if let Some((new_leader_term, view_change_signatures)) = new_view {
            let view_change = ConsensusMessage::ViewChange { new_leader_term };
            self.verify_rpu_majority_signatures(&view_change, &view_change_signatures)?;
            follower_state.leader_term = new_leader_term;
        }

        for block in blocks {
            let mut world_state = self.world_state.get_writable().await;

            if block.body.height <= world_state.block_number {
                continue;
            }

            if block.body.height != world_state.block_number + 1 {
                return Err(Error::BlockNumberDoesNotMatch(
                    block.body.height,
                    world_state.block_number + 1,
                ));
            }

            if block.body.prev_block_hash != world_state.last_block_hash {
                return Err(Error::BlockHashDoesNotMatch(
                    block.body.prev_block_hash,
                    world_state.last_block_hash,
                ));
            }

            // Verify block signatures
            let ackappend_message = ConsensusMessage::AckAppend {
                leader_term: block.body.leader_term,
                block_number: block.body.height,
                block_hash: block.hash(),
            };
            self.verify_rpu_majority_signatures(&ackappend_message, &block.signatures)?;
            let data = &block.body.transactions;
            if data.is_empty() {
                return Err(Error::EmptyBlock);
            }
            // Validate Transactions
            self.transaction_checker.verify_signatures(data)?;

            self.increment_state_and_write_block(&mut follower_state, &block)
                .await;

            // Write Block to WorldState
            world_state.apply_block(block).unwrap();
            world_state.save();
        }

        log::trace!("Done Synchronizing");
        Ok(follower_state)
    }

    pub(super) async fn handle_synchronization_request(
        &self,
        peer_id: &PeerId,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
    ) -> Result<ConsensusMessage, Error> {
        println!("Request by {} to synchornize.", peer_id);
        self.transaction_checker.verify_is_rpu(peer_id)?;
        let (new_view, current_block_number) = {
            let follower_state = self.follower_state.lock().await;

            let new_view = if leader_term == follower_state.leader_term {
                None
            } else {
                Some((
                    follower_state.leader_term,
                    follower_state.new_view_signatures.clone(),
                ))
            };

            (new_view, follower_state.block_number)
        };

        let blocks: Result<_, _> = self
            .block_storage
            .read(block_number..=current_block_number)
            .collect();
        let blocks = blocks?;
        Ok(ConsensusMessage::SynchronizationResponse { new_view, blocks })
    }

    pub(super) fn verify_rpu_majority_signatures(
        &self,
        message: &ConsensusMessage,
        signatures: &SignatureList,
    ) -> Result<(), Error> {
        if !signatures.is_unique() {
            let error = Error::DuplicateSignatures;
            log::error!("{}", error);
            return Err(error);
        }

        if !self.supermajority_reached(signatures.len()) {
            return Err(Error::NotEnoughSignatures);
        }

        for (peer_id, signature) in signatures {
            // All signatures in here must be valid.
            // The leader would filter out any wrong signatures.
            match peer_id.verify(&message, signature) {
                Ok(()) => {}
                Err(err) => {
                    log::error!("Error while verifying signatures: {}", err);
                    return Err(err.into());
                }
            };

            // Also check whether the signer is a known RPU
            self.transaction_checker.verify_is_rpu(peer_id)?;
        }

        Ok(())
    }
}
