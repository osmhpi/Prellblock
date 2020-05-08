use super::{
    super::{BlockHash, BlockNumber, LeaderTerm, SignatureList},
    message::ConsensusMessage,
    state::FollowerState,
    Error, PRaftBFT,
};
use crate::peer::{message as peer_message, Sender};
use pinxit::{PeerId, Signable};
use rand::Rng;
use std::net::SocketAddr;
use tokio::sync::MutexGuard;

const SYNCHRONIZATION_BLOCK_THRESHOLD: u64 = 3;

type SynchronizerGuard<'a> = MutexGuard<'a, ()>;

#[allow(clippy::single_match_else)]
impl PRaftBFT {
    /// Synchronize if there is only one instance of synchronisation running.
    pub(super) async fn synchronize_if_needed(
        &self,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
    ) -> Result<MutexGuard<'_, FollowerState>, Error> {
        let synchronizer_guard = self.synchronizer_lock.lock().await;
        let mut follower_state = self.follower_state.lock().await;
        if self.is_synchronization_needed(&follower_state, leader_term, block_number) {
            follower_state = self.synchronize(synchronizer_guard, follower_state).await?;
        }
        Ok(follower_state)
    }

    /// Check whether we need to synchronize to handle
    /// a request in a given `leader_term` and `block_number`.
    fn is_synchronization_needed(
        &self,
        follower_state: &FollowerState,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
    ) -> bool {
        let _ = self;
        leader_term > follower_state.leader_term
            || block_number >= follower_state.block_number + SYNCHRONIZATION_BLOCK_THRESHOLD
    }

    async fn synchronize(
        &self,
        synchronizer_guard: SynchronizerGuard<'_>,
        follower_state: MutexGuard<'_, FollowerState>,
    ) -> Result<MutexGuard<'_, FollowerState>, Error> {
        // choose peer to ask for synchronization randomly
        // but ensure, we're not sending the request to ourselves
        let peers = self.peers();
        assert_ne!(peers.len(), 1);
        let peer_address = loop {
            let peer_index = rand::thread_rng().gen_range(0, peers.len());
            let peer = &peers[peer_index];
            if &peer.0 != self.peer_id() {
                break peer.1;
            }
        };

        self.synchronize_inner(synchronizer_guard, follower_state, peer_address)
            .await
            .map_err(|err| {
                log::error!("Synchronization error: {}", err);
                err
            })
    }

    pub(super) async fn synchronize_from(
        &self,
        peer_id: &PeerId,
        follower_state: MutexGuard<'_, FollowerState>,
    ) -> Result<MutexGuard<'_, FollowerState>, Error> {
        let synchronizer_guard = self.synchronizer_lock.lock().await;
        if let Some((_, peer_address)) = self.peers().iter().find(|(pid, _)| pid == peer_id) {
            self.synchronize_inner(synchronizer_guard, follower_state, *peer_address)
                .await
                .map_err(|err| {
                    log::error!("Synchronization error: {}", err);
                    err
                })
        } else {
            Err(Error::InvalidPeer(peer_id.clone()))
        }
    }

    async fn synchronize_inner(
        &self,
        synchronizer_guard: SynchronizerGuard<'_>,
        follower_state: MutexGuard<'_, FollowerState>,
        peer_address: SocketAddr,
    ) -> Result<MutexGuard<'_, FollowerState>, Error> {
        let leader_term = follower_state.leader_term;
        let block_number = follower_state.block_number;
        let block_hash = follower_state.last_block_hash();
        drop(follower_state);

        // send request to peer
        let mut sender = Sender::new(peer_address);

        // verify received blocks, if not timeouted
        // append correct blocks to own blockstorage
        let consensus_message = ConsensusMessage::SynchronizationRequest {
            leader_term,
            block_number,
            block_hash,
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

        if let Some(first_block) = blocks.first() {
            if follower_state.rollback_possible
                && first_block.block_number() == block_number
                && first_block.hash() != block_hash
            {
                // We had a chain split.
                self.rollback_last_block(&mut follower_state).await;
            }
        }

        for block in blocks {
            let world_state = self.world_state.get_writable().await;

            if block.body.height <= world_state.block_number {
                continue;
            }

            if block.body.height != world_state.block_number + 1 {
                return Err(Error::PrevBlockNumberDoesNotMatch(
                    block.body.height,
                    world_state.block_number + 1,
                ));
            }

            if block.body.prev_block_hash != world_state.last_block_hash {
                return Err(Error::PrevBlockHashDoesNotMatch(
                    block.body.prev_block_hash,
                    world_state.last_block_hash,
                ));
            }

            // Verify block signatures
            let block_hash = block.hash();
            let ackappend_message = ConsensusMessage::AckAppend {
                leader_term: block.body.leader_term,
                block_number: block.body.height,
                block_hash,
            };
            self.verify_rpu_majority_signatures(&ackappend_message, &block.signatures)?;
            let data = &block.body.transactions;
            if data.is_empty() {
                return Err(Error::EmptyBlock);
            }
            // Validate Transactions
            self.transaction_checker.verify_signatures(data)?;

            // Persist the blocks after all checks have passed.
            self.increment_state_and_write_block(
                &mut follower_state,
                world_state,
                block,
                block_hash,
            )
            .await;
        }

        log::trace!("Done synchronizing.");
        drop(synchronizer_guard);
        Ok(follower_state)
    }

    pub(super) async fn handle_synchronization_request(
        &self,
        peer_id: &PeerId,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
        expected_block_hash: BlockHash,
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

        // Only send all blocks of range block_number..=current_block_number,
        // if the first requested block's hash does not match the sent one.
        let mut blocks_iter = self.block_storage.read(block_number..=current_block_number);

        let first_block = match blocks_iter.next() {
            Some(Ok(first_block)) if expected_block_hash != first_block.hash() => {
                Some(Ok(first_block))
            }
            Some(Err(err)) => Some(Err(err)),
            _ => None,
        };
        let blocks = first_block
            .into_iter()
            .chain(blocks_iter)
            .collect::<Result<_, _>>()?;
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
