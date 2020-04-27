use super::{message, Calculator, Pong};
use crate::{
    consensus::{Consensus, ConsensusMessage},
    data_storage::DataStorage,
    permission_checker::PermissionChecker,
    BoxError,
};
use pinxit::Signed;
use prellblock_client_api::Transaction;
use std::sync::{Arc, Mutex};

type ArcMut<T> = Arc<Mutex<T>>;

/// A `PeerInbox` instance.
pub struct PeerInbox {
    calculator: ArcMut<Calculator>,
    data_storage: Arc<DataStorage>,
    consensus: Arc<Consensus>,
    permission_checker: Arc<PermissionChecker>,
}

impl PeerInbox {
    /// Create a new `PeerInbox` instance.
    #[must_use]
    pub fn new(
        calculator: ArcMut<Calculator>,
        data_storage: Arc<DataStorage>,
        consensus: Arc<Consensus>,
        permission_checker: Arc<PermissionChecker>,
    ) -> Self {
        Self {
            calculator,
            data_storage,
            consensus,
            permission_checker,
        }
    }

    /// Handle an `execute` `Signable` message.
    pub fn handle_execute(&self, params: &Signed<Transaction>) -> Result<(), BoxError> {
        let transaction = params;
        let transaction = transaction.verify_ref()?;

        // Verify permissions
        self.permission_checker
            .verify(transaction.signer(), &transaction)?;

        match &*transaction {
            Transaction::KeyValue { key, value } => {
                // TODO: Deserialize value.
                log::info!(
                    "Client {} set {} to {:?} (via another RPU)",
                    &transaction.signer(),
                    key,
                    value,
                );

                // TODO: Continue with warning or error?
                self.data_storage.write(transaction.signer(), key, value)?;
            }
        }
        Ok(())
    }

    /// Handle a batch of `execute` `Signable` messages.
    pub async fn handle_execute_batch(
        &self,
        params: message::ExecuteBatch,
    ) -> Result<(), BoxError> {
        let message::ExecuteBatch(batch) = params;
        for message in &batch {
            if let Err(err) = self.handle_execute(message) {
                log::error!("Error while handling message: {}", err);
            }
        }
        self.consensus.take_transactions(batch).await;
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
    pub async fn handle_consensus(
        &self,
        params: message::Consensus,
    ) -> Result<Signed<ConsensusMessage>, BoxError> {
        Ok(self.consensus.handle_message(params.0).await?)
    }
}
