use super::{
    message::{consensus_message as message, Metadata},
    Core, Error, Follower, InvalidTransaction, ViewChange, MAX_TRANSACTIONS_PER_BLOCK,
};
use crate::{
    consensus::{BlockHash, BlockNumber, Body, LeaderTerm, SignatureList},
    transaction_checker::TransactionCheck,
};
use pinxit::{verify_signed_batch, Signed};
use prellblock_client_api::Transaction;
use std::{
    ops::Deref,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::time;

const BLOCK_GENERATION_TIMEOUT: Duration = Duration::from_millis(400);

#[derive(Debug)]
pub struct Leader {
    core: Arc<Core>,
    follower: Arc<Follower>,
    view_change: Arc<ViewChange>,
    leader_term: LeaderTerm,
    block_number: BlockNumber,
    last_block_hash: BlockHash,
    phase: Phase,
    /// Represents the leader's internal `WorldState`.
    transaction_check: TransactionCheck,
}

impl Deref for Leader {
    type Target = Core;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

#[derive(Debug)]
enum Phase {
    Waiting,
    Prepare,
    Append,
    Commit,
}

impl Leader {
    pub fn new(core: Arc<Core>, follower: Arc<Follower>, view_change: Arc<ViewChange>) -> Self {
        let transaction_check = core.transaction_checker.check();
        Self {
            core,
            follower,
            view_change,
            leader_term: LeaderTerm::default(),
            block_number: BlockNumber::default(),
            last_block_hash: BlockHash::default(),
            phase: Phase::Waiting,
            transaction_check,
        }
    }

    /// Execute the leader.
    ///
    /// This function waits until it is notified of a leader change.
    pub async fn execute(mut self) {
        loop {
            self.synchronize_from_follower().await;

            // Wait when we are not the leader.
            while !self.is_current_leader() {
                log::trace!("I am not the current leader.");

                // Send new view message.
                self.handle_new_view().await;
                self.notify_leader.notified().await;

                // Update leader state with data from the follower state when we are the new leader.
                self.synchronize_from_follower().await;
            }

            log::info!(
                "I am the new leader in leader term {} (block: #{}).",
                self.leader_term,
                self.block_number
            );
            match self.execute_leader_term().await {
                Ok(()) => log::info!(
                    "Done with leader term {} (block: #{}).",
                    self.leader_term,
                    self.block_number
                ),
                Err(err) => log::error!(
                    "Error during leader term {} (block: #{}, phase: {:?}): {}",
                    self.leader_term,
                    self.block_number,
                    self.phase,
                    err
                ),
            }

            // After we are done with one leader term,
            // we need to wait until the next time we are elected.
            // (At least one round later)
            self.leader_term += 1;
        }
    }

    /// Update the leader state with the state from the follower.
    async fn synchronize_from_follower(&mut self) {
        let state = self.follower.state().await;
        // This `if` is required because we set our `leader_term` to
        // the next value when an error occurs (`self.leader_term += 1`)
        // and we dont want to override this with the state of the follower.
        if self.leader_term <= state.leader_term {
            self.leader_term = state.leader_term;
            self.block_number = state.block_number;
            self.last_block_hash = state.last_block_hash;
        }

        // Update the leader's world state.
        self.transaction_check = self.transaction_checker.check();
    }

    /// Broadcast a `NewView` message of one is available.
    /// Returns `true` if a `NewView` message was sent.
    async fn handle_new_view(&mut self) {
        if let Some(message) = self.view_change.get_new_view_message(self.block_number) {
            let new_leader_term = message.leader_term;
            match self.broadcast_until_majority(message, |_| Ok(())).await {
                Ok(_) => log::trace!(
                    "Succesfully broadcasted NewView Message {}.",
                    new_leader_term,
                ),
                Err(err) => {
                    log::warn!(
                        "Error while Broadcasting NewView Message {}: {}",
                        new_leader_term,
                        err
                    );
                }
            }
        }
    }

    /// Execute the leader during a single leader term.
    ///
    /// This function waits until it is notified to process transactions.
    async fn execute_leader_term(&mut self) -> Result<(), Error> {
        let mut timeout_result = Ok(());
        loop {
            self.phase = Phase::Waiting;

            let min_block_size = match timeout_result {
                // No timeout, send only full blocks
                Ok(()) => MAX_TRANSACTIONS_PER_BLOCK,
                // Timeout, send all pending transactions
                Err(_) => 1,
            };
            while self.queue.lock().await.len() >= min_block_size {
                self.execute_round().await?;
            }
            timeout_result =
                time::timeout(BLOCK_GENERATION_TIMEOUT, self.notify_leader.notified()).await;
        }
    }

    /// Execute the leader during a single round (block number).
    async fn execute_round(&mut self) -> Result<(), Error> {
        let mut transactions = Vec::new();

        // TODO: Check size of transactions cumulated.
        while let Some(transaction) = self.queue.lock().await.next() {
            transactions.push(transaction);

            if transactions.len() >= MAX_TRANSACTIONS_PER_BLOCK {
                break;
            }
        }

        // Also applies valid transactions onto the leader's virutal world state.
        let (valid_transactions, invalid_transactions) = self.stateful_validate(transactions)?;

        let body = Body {
            leader_term: self.leader_term,
            height: self.block_number,
            prev_block_hash: self.last_block_hash,
            timestamp: SystemTime::now(),
            transactions: valid_transactions,
        };

        let block_hash = body.hash();

        let ackprepare_signatures = self.prepare(block_hash).await?;
        log::trace!(
            "Prepare Phase #{} ended. Got ACKPREPARE signatures: {:?}",
            self.block_number,
            ackprepare_signatures,
        );

        let ackappend_signatures = self
            .append(
                block_hash,
                body.transactions,
                invalid_transactions,
                ackprepare_signatures,
                body.timestamp,
            )
            .await?;
        log::trace!(
            "Append Phase #{} ended. Got ACKAPPEND signatures: {:?}",
            self.block_number,
            ackappend_signatures,
        );

        self.commit(block_hash, ackappend_signatures).await?;
        log::info!("Comitted block #{} on majority of RPUs.", self.block_number);

        self.block_number += 1;
        self.last_block_hash = block_hash;

        Ok(())
    }

    async fn prepare(&mut self, block_hash: BlockHash) -> Result<SignatureList, Error> {
        self.phase = Phase::Prepare;

        let metadata = self.metadata_with(block_hash);
        let message = message::Prepare {
            metadata: metadata.clone(),
        };

        self.broadcast_until_majority(message, move |ack| ack.metadata.verify(&metadata))
            .await
    }

    async fn append(
        &mut self,
        block_hash: BlockHash,
        valid_transactions: Vec<Signed<Transaction>>,
        invalid_transactions: Vec<(usize, Signed<Transaction>)>,
        ackprepare_signatures: SignatureList,
        timestamp: SystemTime,
    ) -> Result<SignatureList, Error> {
        self.phase = Phase::Append;

        let metadata = self.metadata_with(block_hash);
        let message = message::Append {
            metadata: metadata.clone(),
            valid_transactions,
            invalid_transactions,
            ackprepare_signatures,
            timestamp,
        };

        self.broadcast_until_majority(message, move |ack| ack.metadata.verify(&metadata))
            .await
    }

    async fn commit(
        &mut self,
        block_hash: BlockHash,
        ackappend_signatures: SignatureList,
    ) -> Result<SignatureList, Error> {
        self.phase = Phase::Commit;

        let metadata = self.metadata_with(block_hash);
        let message = message::Commit {
            metadata: metadata.clone(),
            ackappend_signatures,
        };

        self.broadcast_until_majority(message, move |_| Ok(()))
            .await
    }

    fn is_current_leader(&self) -> bool {
        self.leader(self.leader_term) == *self.identity.id()
    }

    const fn metadata_with(&self, block_hash: BlockHash) -> Metadata {
        Metadata {
            leader_term: self.leader_term,
            block_number: self.block_number,
            block_hash,
        }
    }

    fn stateful_validate(
        &mut self,
        transactions: Vec<Signed<Transaction>>,
    ) -> Result<(Vec<Signed<Transaction>>, Vec<InvalidTransaction>), Error> {
        let verified_transactions = verify_signed_batch(transactions)?;

        let mut valid_transactions = Vec::new();
        let mut invalid_transactions = Vec::new();
        for (index, transaction) in verified_transactions.enumerate() {
            // This applies valid transaction to the leader's own world state.
            if self
                .transaction_check
                .verify_permissions_and_apply(transaction.borrow())
                .is_ok()
            {
                valid_transactions.push(transaction.into());
            } else {
                invalid_transactions.push((index, transaction.into()));
            }
        }

        Ok((valid_transactions, invalid_transactions))
    }
}
