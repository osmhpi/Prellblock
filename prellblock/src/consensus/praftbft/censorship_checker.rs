use super::{Core, ViewChange};
use std::{ops::Deref, sync::Arc, time::Duration};
use tokio::time;

// After this amount of time a transaction should be committed.
const CENSORSHIP_TIMEOUT: Duration = Duration::from_secs(10);

pub struct CensorshipChecker {
    core: Arc<Core>,
    view_change: Arc<ViewChange>,
}

impl Deref for CensorshipChecker {
    type Target = Core;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl CensorshipChecker {
    pub fn new(core: Arc<Core>, view_change: Arc<ViewChange>) -> Self {
        Self { core, view_change }
    }

    /// Execute the censorship checker.
    ///
    /// This is woken up after a timeout or a specific
    /// number of blocks commited.
    pub async fn execute(self) {
        loop {
            let timeout_result = time::timeout(
                CENSORSHIP_TIMEOUT,
                self.notify_censorship_checker.notified(),
            )
            .await;

            // If there was no timeout, a leader change happened.
            // Give the leader enough time by sleeping again.
            if timeout_result.is_ok() {
                continue;
            }

            // Checking only the first transaction,
            // the queue is already sorted by insertion time.
            let has_old_transactions = self.queue.lock().await.peek().map_or(false, |entry| {
                entry.inserted().elapsed() > CENSORSHIP_TIMEOUT
            });

            if has_old_transactions {
                // leader seems to be faulty / dead or censoring
                log::warn!("Found censored transactions. Requesting View Change.",);
                self.view_change.request_view_change().await;
            } else {
                log::trace!("No old transactions found while checking for censorship.");
            }
        }
    }
}
