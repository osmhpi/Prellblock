use super::{message, Calculator, Pong};
use crate::{
    consensus::{Consensus, ConsensusResponse},
    data_storage::DataStorage,
    transaction_checker::TransactionChecker,
    BoxError,
};
use pinxit::Signed;
use prellblock_client_api::Transaction;
use rayon::prelude::*;
use std::sync::{Arc, Mutex};

type ArcMut<T> = Arc<Mutex<T>>;

/// A `PeerInbox` instance.
pub struct PeerInbox {
    calculator: ArcMut<Calculator>,
    data_storage: Arc<DataStorage>,
    consensus: Arc<Consensus>,
    transaction_checker: Arc<TransactionChecker>,
}

impl PeerInbox {
    /// Create a new `PeerInbox` instance.
    #[must_use]
    pub fn new(
        calculator: ArcMut<Calculator>,
        data_storage: Arc<DataStorage>,
        consensus: Arc<Consensus>,
        transaction_checker: Arc<TransactionChecker>,
    ) -> Self {
        Self {
            calculator,
            data_storage,
            consensus,
            transaction_checker,
        }
    }

    /// Handle an `execute` `Signable` message.
    pub fn handle_execute(&self, params: &Signed<Transaction>) -> Result<(), BoxError> {
        let transaction = params;
        let transaction = transaction.verify_ref()?;

        // Verify permissions
        self.transaction_checker
            .verify_permissions(transaction.signer(), &transaction)?;

        match &*transaction {
            Transaction::KeyValue { key, value } => {
                // TODO: Deserialize value.
                log::debug!(
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

        // Parallel verification makes it somewhat faster.
        let result = batch
            .par_iter()
            .map(|message| self.handle_execute(message))
            .collect::<Result<(), BoxError>>();
        if let Err(err) = result {
            log::error!("Error while handling message: {}", err);
        }

        let consensus = self.consensus.clone();
        // This would otherwise block the batcher on the sending side
        // because taking the transactions could take a while...
        tokio::spawn(async move { consensus.take_transactions(batch).await });
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
    ) -> Result<Signed<ConsensusResponse>, BoxError> {
        Ok(self.consensus.handle_message(params.0).await?)
    }
}
