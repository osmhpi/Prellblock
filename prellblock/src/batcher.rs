//! Module used for batching messages for a `Broadcaster`.

use crate::{data_broadcaster::Broadcaster, peer::message};
use pinxit::Signed;
use prellblock_client_api::Transaction;
use std::{mem, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, Mutex},
    time::timeout,
};

const MAX_TRANSACTIONS_PER_BATCH: usize = 4000;
const MAX_TIME_BETWEEN_BATCHES: Duration = Duration::from_millis(400);

/// A Batcher for messages.
pub struct Batcher {
    broadcaster: Arc<Broadcaster>,
    bucket: Mutex<Vec<Signed<Transaction>>>,
    notifier: mpsc::Sender<()>,
}

impl Batcher {
    /// Create a new Batcher instance. `broadcaster` needs to be of type `Arc<prellblock::data_broadcaster::Broadcaster>`.
    #[must_use]
    pub fn new(broadcaster: Arc<Broadcaster>) -> Arc<Self> {
        let (notifier, receiver) = mpsc::channel(1);
        let batcher = Self {
            broadcaster,
            bucket: Mutex::default(),
            notifier,
        };
        let batcher = Arc::new(batcher);
        {
            let batcher = batcher.clone();
            tokio::spawn(batcher.periodically_send_to_broadcaster(receiver));
        }
        batcher
    }

    /// Add a received message to the batchers bucket.
    pub async fn add_to_batch(self: Arc<Self>, transaction: Signed<Transaction>) {
        let mut bucket = self.bucket.lock().await;
        bucket.push(transaction);
        if bucket.len() >= MAX_TRANSACTIONS_PER_BATCH {
            log::trace!("Filled bucket.");
            let result = self.notifier.clone().try_send(());
            if let Err(mpsc::error::TrySendError::Closed(_)) = result {
                panic!("The broadcaster task is not running.");
            }
        }
    }

    async fn periodically_send_to_broadcaster(self: Arc<Self>, mut receiver: mpsc::Receiver<()>) {
        loop {
            let timeout_result = timeout(MAX_TIME_BETWEEN_BATCHES, receiver.recv()).await;
            let mut was_timeout = false;
            if let Ok(None) = timeout_result {
                // It was nice to know you. Goodbye.
                break;
            } else if timeout_result.is_err() {
                was_timeout = true;
            }

            let transactions = mem::take(&mut *self.bucket.lock().await);
            if transactions.is_empty() {
                continue;
            }
            log::trace!(
                "Start sending batch with {} transactions (Timeout: {}).",
                transactions.len(),
                was_timeout
            );

            let message = message::ExecuteBatch(transactions);
            let broadcaster = self.broadcaster.clone();
            tokio::spawn(async move {
                match broadcaster.broadcast(&message).await {
                    Ok(_) => log::debug!("Batch sent successfully"),
                    Err(err) => log::error!("Error sending batch: {}", err),
                }
            });
        }
    }
}
