use super::{super::message::Metadata, message, response, Error, ErrorVerify, Follower, State};
use crate::consensus::{Block, BlockNumber, LeaderTerm};
use balise::Address;
use pinxit::PeerId;
use rand::Rng;
use tokio::sync::{MutexGuard, SemaphorePermit};

const SYNCHRONIZATION_BLOCK_THRESHOLD: u64 = 3;

impl Follower {
    /// Synchronize if there is only one instance of synchronisation running.
    pub async fn synchronize_if_needed(
        &self,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
    ) -> Result<(), Error> {
        let synchronizer_permit = self.synchronizer_semaphore.acquire().await;
        let synchronizer_permit = synchronizer_permit.expect("unable to acquire");
        let state = self.state.lock().await;

        if self.is_synchronization_needed(&state, leader_term, block_number) {
            // choose peer to ask for synchronization randomly
            // but ensure, we're not sending the request to ourselves
            let peers = self.world_state.get().peers;
            assert_ne!(peers.len(), 1);
            let peer_address = loop {
                let peer_index = rand::thread_rng().gen_range(0, peers.len());
                let peer = &peers[peer_index];
                if peer.0 != *self.identity.id() {
                    break &peer.1;
                }
            };

            self.synchronize(synchronizer_permit, state, peer_address.clone())
                .await
                .map_err(|err| {
                    log::error!("Synchronization error: {}", err);
                    err
                })?;
        }

        Ok(())
    }

    /// Check whether we need to synchronize to handle
    /// a request in a given `leader_term` and `block_number`.
    fn is_synchronization_needed(
        &self,
        state: &State,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
    ) -> bool {
        let _ = self;
        leader_term > state.leader_term
            || block_number >= state.block_number + SYNCHRONIZATION_BLOCK_THRESHOLD
    }

    pub async fn synchronize_from(&self, peer_id: &PeerId) -> Result<MutexGuard<'_, State>, Error> {
        let synchronizer_permit = self.synchronizer_semaphore.acquire().await;
        let synchronizer_permit = synchronizer_permit.expect("unable to acquire");
        if let Some((_, peer_address)) = self
            .world_state
            .get()
            .peers
            .iter()
            .find(|(pid, _)| pid == peer_id)
        {
            let state = self.state.lock().await;
            self.synchronize(synchronizer_permit, state, peer_address.clone())
                .await
                .map_err(|err| {
                    log::error!("Synchronization error: {}", err);
                    err
                })
        } else {
            Err(Error::InvalidPeer(peer_id.clone()))
        }
    }

    async fn synchronize(
        &self,
        synchronizer_permit: SemaphorePermit<'_>,
        state: MutexGuard<'_, State>,
        peer_address: Address,
    ) -> Result<MutexGuard<'_, State>, Error> {
        let request = message::SynchronizationRequest {
            leader_term: state.leader_term,
            block_number: state.block_number,
            block_hash: state.last_block_hash,
        };
        drop(state);

        log::trace!(
            "Trying to synchronize from {} with: {:#?}",
            peer_address,
            request
        );

        // send request to peer
        let response = self.send_message(peer_address, request).await?.into_inner();

        let mut state = self.state.lock().await;
        if let Some((new_leader_term, view_change_signatures)) = response.new_view {
            self.verify_rpu_majority_signatures(
                message::ViewChange { new_leader_term },
                &view_change_signatures,
            )?;
            state.new_leader_term(new_leader_term, view_change_signatures);
        }

        if let Some(first_block) = response.blocks.first() {
            if state.rollback_possible
                && first_block.block_number() + 1 == state.block_number
                && first_block.hash() != state.last_block_hash
            {
                // We had a chain split.
                log::trace!("Doing rollback.");
                state.rollback().await;
                log::trace!("Done rollback.");
            }
        }

        // verify received blocks, if not timeouted
        // append correct blocks to own blockstorage
        log::trace!(
            "Received {} blocks while synchronizing.",
            response.blocks.len()
        );
        for block in response.blocks {
            log::trace!("Applying synchronized block: {:#?}", block);
            if block.body.height < state.block_number {
                continue;
            }

            self.apply_synchronized_block(&mut state, block).await?;
        }

        log::trace!("Done synchronizing.");
        drop(synchronizer_permit);
        Ok(state)
    }

    async fn apply_synchronized_block(&self, state: &mut State, block: Block) -> Result<(), Error> {
        block.body.height.verify(state.block_number)?;

        if block.body.prev_block_hash != state.last_block_hash {
            return Err(Error::PrevBlockHashDoesNotMatch(
                block.body.prev_block_hash,
                state.last_block_hash,
            ));
        }

        // Verify block signatures
        let block_hash = block.hash();
        self.verify_rpu_majority_signatures(
            response::AckAppend {
                metadata: Metadata {
                    leader_term: block.body.leader_term,
                    block_number: block.body.height,
                    block_hash,
                },
            },
            &block.signatures,
        )?;

        let data = &block.body.transactions;
        if data.is_empty() {
            return Err(Error::EmptyBlock);
        }

        // Validate Transactions
        self.transaction_checker.verify(data)?;

        // Persist the blocks after all checks have passed.
        state.apply_block(block_hash, block).await;

        Ok(())
    }

    pub async fn handle_synchronization_request(
        &self,
        peer_id: PeerId,
        message: message::SynchronizationRequest,
    ) -> Result<response::SynchronizationResponse, Error> {
        log::trace!("Request by {} to synchronize.", peer_id);

        let (new_view, current_block_number) = {
            let state = self.state.lock().await;

            let new_view = if message.leader_term == state.leader_term {
                None
            } else {
                Some((state.leader_term, state.new_view_signatures.clone()))
            };

            (new_view, state.block_number)
        };

        log::trace!(
            "Synchronizer is at new_view {:?} block_nummer {}.",
            new_view,
            current_block_number
        );

        // Only send the block `message.block_number`
        // if the first requested block's hash does not match the sent one.
        let mut start_block_number = message.block_number;
        if start_block_number > BlockNumber::default() {
            start_block_number -= 1;
        }
        let mut blocks_iter = self
            .block_storage
            .read(start_block_number..=current_block_number);

        let first_block = match blocks_iter.next() {
            Some(Ok(first_block)) if message.block_hash != first_block.hash() => {
                Some(Ok(first_block))
            }
            Some(Err(err)) => Some(Err(err)),
            _ => None,
        };
        log::trace!("First block being sent to follower: {:#?}", first_block);
        let blocks = first_block
            .into_iter()
            .chain(blocks_iter)
            .collect::<Result<Vec<_>, _>>()?;
        log::trace!("Sending {} blocks to {}.", blocks.len(), peer_id);
        Ok(response::SynchronizationResponse { new_view, blocks })
    }
}
