use super::{message, Core, Error, InvalidTransaction, NotifyMap};
use crate::{
    consensus::{Block, BlockHash, BlockNumber, Body, LeaderTerm, SignatureList},
    if_monitoring,
};
use pinxit::{PeerId, Signed};
use prellblock_client_api::Transaction;
use std::{ops::Deref, sync::Arc, time::SystemTime};
if_monitoring! {
    use super::{
        QUEUE_BACKLOG, QUEUE_RESIDENCE_TIME,
    };
}

#[derive(Debug)]
pub struct State {
    pub core: Arc<Core>,

    /// The current leader term.
    pub leader_term: LeaderTerm,
    /// The signatures from the `NewView` message.
    pub new_view_signatures: SignatureList,

    /// A notifier to notify taks once we reached a given block number.
    pub block_changed: NotifyMap<BlockNumber>,
    /// The number of the current block.
    pub block_number: BlockNumber,
    /// The hash of the last block.
    pub last_block_hash: BlockHash,
    /// The hash of the current block. (Set in prepare phase)
    pub block_hash: Option<BlockHash>,
    /// The body of the current block. (Set in append phase)
    pub block_content: Option<(Body, Vec<InvalidTransaction>)>,
    /// Wheter an rollback is currently allowed (only once after a leader change)
    pub rollback_possible: bool,

    /// An out-of-order commit message. (Set in prepare phase during handle commit)
    pub buffered_commit_message: Option<message::Commit>,
}

impl Deref for State {
    type Target = Core;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Waiting,
    Prepare,
    Append,
}

impl State {
    pub fn new(core: Arc<Core>) -> Self {
        let world_state = core.world_state.get();
        Self {
            core,
            leader_term: LeaderTerm::default(),
            new_view_signatures: SignatureList::default(),
            block_changed: NotifyMap::default(),
            block_number: world_state.block_number,
            last_block_hash: world_state.last_block_hash,
            block_hash: None,
            block_content: None,
            rollback_possible: world_state.block_number > BlockNumber::default(),
            buffered_commit_message: None,
        }
    }

    /// Get the current phase.
    pub fn phase(&self) -> Phase {
        match (&self.block_hash, &self.block_content) {
            (None, None) => Phase::Waiting,
            (Some(_), None) => Phase::Prepare,
            (Some(_), Some(_)) => Phase::Append,
            (None, Some(_)) => unreachable!(),
        }
    }

    /// Verify whether the given `peer_id` is the current leader.
    pub fn verify_leader(&self, peer_id: &PeerId) -> Result<(), Error> {
        if self.leader(self.leader_term) == *peer_id {
            Ok(())
        } else {
            Err(Error::WrongLeader(peer_id.clone()))
        }
    }

    /// Create a body with the given `transactions`.
    pub fn body_with(&self, transactions: Vec<Signed<Transaction>>, timestamp: SystemTime) -> Body {
        Body {
            leader_term: self.leader_term,
            height: self.block_number,
            prev_block_hash: self.last_block_hash,
            timestamp,
            transactions,
        }
    }

    /// Create a body signed by `ackappend_signatures`.
    fn block_with(&self, ackappend_signatures: SignatureList) -> Block {
        Block {
            body: self.block_content.as_ref().unwrap().0.clone(),
            signatures: ackappend_signatures,
        }
    }

    /// Move to the prepare phase.
    ///
    /// Panics if not in waiting phase.
    pub fn prepare(&mut self, block_hash: BlockHash) {
        assert_eq!(self.phase(), Phase::Waiting);
        self.block_hash = Some(block_hash);
    }

    /// Move to the append phase.
    ///
    /// Panics if not in prepare phase.
    pub fn append(&mut self, body: Body, invalid_transactions: Vec<InvalidTransaction>) {
        assert_eq!(self.phase(), Phase::Prepare);
        self.block_content = Some((body, invalid_transactions))
    }

    /// Commit a block using a list of ackappend `signatures`.
    ///
    /// Panics if not in append phase.
    pub async fn commit(&mut self, ackappend_signatures: SignatureList) {
        // Unwrap of `block_hash` and `block_content` should be safe
        // because we assert being in the Append phase.
        assert_eq!(self.phase(), Phase::Append);
        assert!(self.buffered_commit_message.is_none());

        let block = self.block_with(ackappend_signatures);
        log::trace!(
            "Committing block #{} with {} transactions.",
            block.body.height,
            block.body.transactions.len()
        );
        let block_hash = self.block_hash.take().unwrap();

        // We are sure that these transactions are really invalid and therefore
        // they can be removed from the queue without losing good transactions.
        let (_, invalid_transactions) = self.block_content.as_ref().unwrap();
        if !invalid_transactions.is_empty() {
            log::warn!(
                "Removing invalid transactions from queue: {:#?}",
                invalid_transactions
            );
            self.queue
                .lock()
                .await
                .remove_all(invalid_transactions.iter().map(|(_, tx)| tx));
        }

        // Must be called at last because it resets the state.
        self.apply_block(block_hash, block).await;
    }

    /// Applies a given block to the state.
    ///
    /// Panics if the block does not match the current block number.
    pub async fn apply_block(&mut self, block_hash: BlockHash, block: Block) {
        assert_eq!(block.block_number(), self.block_number);

        // Remove committed transactions from our queue.
        let removed_txs = self
            .queue
            .lock()
            .await
            .remove_all(block.body.transactions.iter());

        if_monitoring!({
            QUEUE_BACKLOG.set(self.queue.lock().await.len() as i64);
            for removed_tx in removed_txs {
                let residence_time = removed_tx.inserted().elapsed().as_secs_f64();
                QUEUE_RESIDENCE_TIME.observe(residence_time);
            }
        });

        // Applies block.
        self.transaction_applier.apply_block(block).await;

        // Setup next round.
        self.block_number += 1;
        self.last_block_hash = block_hash;
        self.block_content = None;
        // No rollback possible after one commit.
        self.rollback_possible = false;

        self.buffered_commit_message = None;

        // Notify waiting tasks
        self.block_changed.notify_all(&self.block_number);
    }

    /// Set a new `leader_term`.
    pub fn new_leader_term(&mut self, leader_term: LeaderTerm, new_view_signatures: SignatureList) {
        self.leader_term = leader_term;
        self.new_view_signatures = new_view_signatures;

        self.block_hash = None;
        self.block_content = None;
        self.rollback_possible = true;

        self.buffered_commit_message = None;

        // On view change, we need to drop all messages from the
        // old leader to allow the new one to send new messages.
        assert_eq!(self.phase(), Phase::Waiting);
    }

    /// Rollback the last commited block.
    ///
    /// Panics if no rollback is possible
    /// or the rollback has an unexpected `block_number`.
    pub async fn rollback(&mut self) {
        assert!(self.rollback_possible);

        // BlockStorage remove topmost block.
        // Double Unwrap should be fine because there needs to be some block.
        let last_block = self.block_storage.pop_block().unwrap().unwrap();
        assert_eq!(last_block.block_number() + 1, self.block_number);

        // The transactions may not be lost.
        self.queue.lock().await.extend(last_block.body.transactions);

        // We ignore all invalid transactions during rolllback. They will be lost.
        // (They would be lost anyway after a restart.)
        // If there are two peers left having these transactions in their queue, they
        // should be able to be elected as leader and propose the "lost" transactions.

        // Rollback WorldState by one block.
        self.world_state.rollback().unwrap();
        let world_state = self.world_state.get();
        self.block_number -= 1;
        assert_eq!(world_state.block_number, self.block_number);

        // Reset State
        self.last_block_hash = world_state.last_block_hash;
        self.block_hash = None;
        self.block_content = None;
        // better save than sorry
        self.rollback_possible = false;

        self.buffered_commit_message = None;
    }
}
