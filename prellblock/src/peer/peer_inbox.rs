use super::{message, Calculator, Pong};
use crate::{
    consensus::{Consensus, ConsensusResponse},
    data_storage::DataStorage,
    transaction_checker::TransactionChecker,
    BoxError,
};
use pinxit::{verify_signed_batch_ref, Signed, VerifiedRef};
use prellblock_client_api::Transaction;
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
    pub fn handle_execute(&self, transaction: VerifiedRef<Transaction>) -> Result<(), BoxError> {
        // Verify permissions
        self.transaction_checker
            .verify_permissions(transaction.signer(), transaction)?;

        match &*transaction {
            Transaction::KeyValue(params) => {
                // TODO: Deserialize value.
                log::debug!(
                    "Client {} set {} to {:?} (via another RPU)",
                    &transaction.signer(),
                    params.key,
                    params.value,
                );

                // TODO: Continue with warning or error?
                self.data_storage
                    .write(transaction.signer(), &params.key, &params.value)?;
            }
            Transaction::UpdateAccount(params) => {
                log::debug!(
                    "Client {} updates account {}:\
                    |---- is_rpu: {:?}\
                    |---- writing_right: {:?}\
                    |---- reading_rights: {:?}",
                    &transaction.signer(),
                    params.id,
                    params.is_rpu,
                    params.has_writing_rights,
                    params.reading_rights,
                );

                // TODO: Write in data storage. Maybe use hardcoded key like "AccountUpdate"
                // self.data_storage
                //     .write(transaction.signer(), &params.key, &params)?;
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

        // Batch verification makes it somewhat faster.
        let verified = verify_signed_batch_ref(&batch)?;
        for message in verified {
            self.handle_execute(message)?;
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
