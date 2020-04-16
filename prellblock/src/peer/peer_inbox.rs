use super::{message, Calculator, Pong, SignedTransaction};
use crate::{
    consensus::{Consensus, ConsensusMessage},
    data_storage::DataStorage,
    BoxError,
};
use pinxit::{PeerId, Signed};
use prellblock_client_api::Transaction;
use std::sync::{Arc, Mutex};

type ArcMut<T> = Arc<Mutex<T>>;

/// A `PeerInbox` instance.
pub struct PeerInbox {
    calculator: ArcMut<Calculator>,
    data_storage: Arc<DataStorage>,
    consensus: Arc<Consensus>,
}

impl PeerInbox {
    /// Create a new `PeerInbox` instance.
    #[must_use]
    pub fn new(
        calculator: ArcMut<Calculator>,
        data_storage: Arc<DataStorage>,
        consensus: Arc<Consensus>,
    ) -> Self {
        Self {
            calculator,
            data_storage,
            consensus,
        }
    }

    /// Handle an `execute` `Signable` message.
    pub fn handle_execute(&self, params: &SignedTransaction) -> Result<(), BoxError> {
        let (peer_id, transaction) = params;
        let transaction = transaction.verify_ref(peer_id)?;

        match &*transaction {
            Transaction::KeyValue { key, value } => {
                log::info!(
                    "Client {} set {} to {} (via another RPU)",
                    &peer_id,
                    key,
                    value
                );

                // TODO: Continue with warning or error?
                self.data_storage.write(peer_id, key, value)?;
            }
        }
        Ok(())
    }

    /// Handle a batch of `execute` `Signable` messages.
    pub fn handle_execute_batch(&self, params: message::ExecuteBatch) -> Result<(), BoxError> {
        let message::ExecuteBatch(batch) = params;
        for message in &batch {
            if let Err(err) = self.handle_execute(message) {
                log::error!("Error while handling message: {}", err);
            }
        }
        self.consensus.take_transactions(batch);
        Ok(())
    }

    /// Handle an add `Add` message, return a `usize` as a `Result`.
    pub fn handle_add(&self, params: &message::Add) -> Result<usize, BoxError> {
        Ok(self.calculator.lock().unwrap().add(params.0, params.1))
    }

    /// Handle a `Sub` message, return a `usize` as a `Result`.
    pub fn handle_sub(&self, params: &message::Sub) -> Result<usize, BoxError> {
        Ok(self.calculator.lock().unwrap().sub(params.0, params.1))
    }

    /// Handle a `ping` message, answer with a `pong` as a `Result`.
    pub fn handle_ping(&self) -> Result<Pong, BoxError> {
        let _ = self;
        Ok(Pong)
    }

    /// Forward messages to the consensus algorithm.
    pub fn handle_consensus(
        &self,
        params: message::Consensus,
    ) -> Result<(PeerId, Signed<ConsensusMessage>), BoxError> {
        Ok(self.consensus.handle_message(params.0, params.1)?)
    }
}
