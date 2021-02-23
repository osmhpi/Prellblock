mod state;
mod stateful_validation;
mod synchronizer;

pub use state::Phase;

use super::{
    message::{consensus_message as message, consensus_response as response},
    Core, Error, ErrorVerify, InvalidTransaction, NotifyMap, ViewChange,
};
use crate::consensus::{BlockNumber, LeaderTerm};
use pinxit::PeerId;
use state::State;
use std::{cmp::Ordering, ops::Deref, sync::Arc};
use tokio::sync::{Mutex, MutexGuard, Semaphore};

#[derive(Debug)]
pub struct Follower {
    core: Arc<Core>,
    view_change: Arc<ViewChange>,
    state: Mutex<State>,
    synchronizer_semaphore: Semaphore,
}

impl Deref for Follower {
    type Target = Core;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl Follower {
    pub fn new(core: Arc<Core>, view_change: Arc<ViewChange>) -> Self {
        Self {
            core: core.clone(),
            view_change,
            state: Mutex::new(State::new(core)),
            synchronizer_semaphore: Semaphore::new(1),
        }
    }

    pub async fn state(&self) -> impl Deref<Target = State> + '_ {
        self.state.lock().await
    }

    /// Wait until we reached the block number the message is at.
    async fn state_in_block(
        &self,
        leader_term: LeaderTerm,
        block_number: BlockNumber,
    ) -> Result<MutexGuard<'_, State>, Error> {
        self.synchronize_if_needed(leader_term, block_number)
            .await?;

        loop {
            let mut state = self.state.lock().await;
            if state.block_number >= block_number {
                break Ok(state);
            }
            let wait = state.block_changed.wait(block_number);
            drop(state);
            wait.await;
        }
    }

    pub async fn handle_prepare_message(
        &self,
        peer_id: PeerId,
        message: message::Prepare,
    ) -> Result<response::AckPrepare, Error> {
        let mut state = self
            .state_in_block(message.leader_term, message.block_number)
            .await?;

        log::trace!("Handle Prepare message #{}.", message.block_number);

        // Check whether the state for the block is Waiting.
        // We only allow to receive messages once.
        state.phase().verify(Phase::Waiting)?;
        message.leader_term.verify(state.leader_term)?;
        state.verify_leader(&peer_id)?;
        message.block_number.verify(state.block_number)?;

        // All checks passed, update our state.
        state.prepare(message.block_hash);

        // Send AckPrepare to the leader.
        // *Note*: Technically, we only need to send a signature of
        // the PREPARE message.
        Ok(response::AckPrepare {
            metadata: message.metadata,
        })
    }

    pub async fn handle_append_message(
        &self,
        peer_id: PeerId,
        message: message::Append,
    ) -> Result<response::AckAppend, Error> {
        let mut state = self
            .state_in_block(message.leader_term, message.block_number)
            .await?;

        log::trace!("Handle Append message #{}.", message.block_number);

        // Check whether the state for the block is Prepare.
        // We only allow to receive messages once.
        match state.phase() {
            Phase::Waiting | Phase::Prepare => {}
            phase => return Err(phase.error(Phase::Prepare)),
        }

        message.leader_term.verify(state.leader_term)?;
        state.verify_leader(&peer_id)?;
        message.block_number.verify(state.block_number)?;

        let metadata = message.metadata.clone();
        let (body, invalid_transactions) = self
            .view_change
            .request_view_change_on_error(async {
                // Validate the Block Hash.
                let block_hash = message.block_hash;
                let body = state.body_with(message.valid_transactions, message.timestamp);
                if body.hash() != block_hash {
                    return Err(Error::BlockNotMatchingHash);
                }

                if let Some(expected_block_hash) = state.block_hash {
                    if block_hash != expected_block_hash {
                        return Err(Error::ChangedBlockHash);
                    }
                } else {
                    state.block_hash = Some(block_hash);
                }

                // Check validity of ACKPREPARE Signatures.
                self.verify_rpu_majority_signatures(
                    response::AckPrepare {
                        metadata: message.metadata.clone(),
                    },
                    &message.ackprepare_signatures,
                )?;

                if body.transactions.is_empty() {
                    // Empty blocks are not allowed.
                    return Err(Error::EmptyBlock);
                }

                // Check for transaction validity.
                self.stateful_validate(&body.transactions, &message.invalid_transactions)?;

                Ok((body, message.invalid_transactions))
            })
            .await?;

        // All checks passed, update our state.
        state.append(body, invalid_transactions);

        // There could be a commit message for this block number that arrived first.
        // We then need to apply the commit (or at least check).
        if let Some(commit_message) = state.buffered_commit_message.take() {
            drop(state);
            match self.handle_commit_message(peer_id, commit_message).await {
                Ok(_) => log::debug!("Used out-of-order commit."),
                Err(err) => log::debug!("Failed to apply out-of-order commit: {}", err),
            }
        }

        Ok(response::AckAppend { metadata })
    }

    pub async fn handle_commit_message(
        &self,
        peer_id: PeerId,
        message: message::Commit,
    ) -> Result<response::Ok, Error> {
        let mut state = self
            .state_in_block(message.leader_term, message.block_number)
            .await?;

        log::trace!("Handle Commit message #{}.", message.block_number);

        message.leader_term.verify(state.leader_term)?;
        state.verify_leader(&peer_id)?;
        message.block_number.verify(state.block_number)?;

        if let Some(expected_block_hash) = state.block_hash {
            if message.block_hash != expected_block_hash {
                return Err(Error::ChangedBlockHash);
            }
        }

        // Check whether the state for the block is Append.
        // We only allow to receive messages once.
        match state.phase() {
            Phase::Waiting | Phase::Prepare if state.buffered_commit_message.is_none() => {
                log::trace!(
                    "Received out-of-order commit message #{}.",
                    message.block_number
                );

                state.block_hash = Some(message.block_hash);
                state.buffered_commit_message = Some(message);
                return Ok(response::Ok);
            }
            Phase::Append => {
                state.block_hash = Some(message.block_hash);
            }
            phase => return Err(phase.error(Phase::Append)),
        }

        self.view_change
            .request_view_change_on_error(async {
                // Check validity of ACKAPPEND Signatures.
                self.verify_rpu_majority_signatures(
                    response::AckAppend {
                        metadata: message.metadata.clone(),
                    },
                    &message.ackappend_signatures,
                )?;

                Ok(())
            })
            .await?;

        // Write Block to WorldState
        state.commit(message.ackappend_signatures).await;

        Ok(response::Ok)
    }

    pub async fn handle_new_view_message(
        &self,
        peer_id: PeerId,
        message: message::NewView,
    ) -> Result<response::Ok, Error> {
        log::trace!("Received NewView Message.");

        let mut state = self.state.lock().await;

        let ordering = message.current_block_number.cmp(&state.block_number);

        if ordering == Ordering::Greater {
            // If the leader's block_number is higher than our's we need to synchronize.
            drop(state);
            state = self.synchronize_from(&peer_id).await?;
        }

        // Only higher leader terms than current one accepted
        if message.leader_term <= state.leader_term {
            return Err(Error::LeaderTermTooSmall(message.leader_term));
        }

        // Leader must be the next peer from the peers list.
        if self.leader(message.leader_term) != peer_id {
            log::warn!(
                "ID: {} is not the correct leader for the new term {}.",
                peer_id,
                message.leader_term
            );
            return Err(Error::WrongLeader(peer_id.clone()));
        }

        // Validate all signatures to ensure the ViewChange ist valid.
        self.verify_rpu_majority_signatures(
            message::ViewChange {
                new_leader_term: message.leader_term,
            },
            &message.view_change_signatures,
        )?;

        if ordering == Ordering::Less {
            drop(state);

            // If the leader is out of date, trigger a ViewChange Message.
            self.view_change
                .request_view_change_in_leader_term(message.leader_term)
                .await;
        } else {
            // We are fine
            self.new_leader_term(&mut state, message);
        }

        Ok(response::Ok)
    }

    fn new_leader_term(&self, state: &mut State, message: message::NewView) {
        self.view_change.new_view_received(message.leader_term);

        if message.leader_term <= state.leader_term {
            log::warn!(
                "Tried to change to leader term {} from leader term {} (ignored).",
                state.leader_term,
                message.leader_term
            );
        } else {
            // Update the leader_term of the state to the new leader_term
            log::debug!(
                "Changed from leader term {} to leader term {}.",
                state.leader_term,
                message.leader_term
            );

            state.new_leader_term(message.leader_term, message.view_change_signatures);

            // The leader can start it's work.
            self.notify_leader.notify_one();
        }
    }
}
