//! Module used for batching messages for a `Broadcaster`.

#![allow(clippy::mutex_atomic)]

use crate::{
    data_broadcaster::Broadcaster,
    peer::{message, SignedTransaction},
};
use std::{
    mem,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

const MAX_TRANSACTIONS_PER_BATCH: usize = 5;
const MAX_TIME_BETWEEN_BATCHES: Duration = Duration::from_secs(1);

/// A Batcher for messages.
pub struct Batcher {
    bucket: Mutex<Bucket>,
    epoch: Epoch,
}

struct Bucket {
    broadcaster: Arc<Broadcaster>,
    bucket: Vec<SignedTransaction>,
}

struct Epoch {
    epoch: Mutex<usize>,
    cvar: Condvar,
}

impl Batcher {
    /// Create a new Batcher instance. `broadcaster` needs to be of type `Arc<prellblock::data_broadcaster::Broadcaster>`.
    #[must_use]
    pub fn new(broadcaster: Arc<Broadcaster>) -> Self {
        Self {
            bucket: Mutex::new(Bucket {
                broadcaster,
                bucket: vec![],
            }),
            epoch: Epoch {
                epoch: Mutex::new(0),
                cvar: Condvar::new(),
            },
        }
    }

    /// Add a received message to the batchers bucket.
    pub fn add_to_batch(self: Arc<Self>, transaction: SignedTransaction) {
        let mut bucket = self.bucket.lock().unwrap();
        let shared_self = self.clone();
        if bucket.bucket.is_empty() {
            thread::spawn(move || {
                shared_self.epoch.next_after_timeout(|| {
                    shared_self.bucket.lock().unwrap().send_to_broadcaster()
                });
            });
        }
        bucket.bucket.push(transaction);
        if bucket.bucket.len() >= MAX_TRANSACTIONS_PER_BATCH {
            self.epoch.next(|| bucket.send_to_broadcaster());
        }
    }
}

impl Bucket {
    fn send_to_broadcaster(&mut self) {
        let bucket = mem::take(&mut self.bucket);
        if bucket.is_empty() {
            return;
        }
        let message = message::ExecuteBatch(bucket);
        let shared_broadcaster = self.broadcaster.clone();
        thread::spawn(move || {
            match shared_broadcaster.broadcast(&message) {
                Ok(_) => log::debug!("Batch sent successfully"),
                Err(err) => log::error!("Error while sending Batch: {}", err),
            };
        });
    }
}

impl Epoch {
    fn next(&self, f: impl FnOnce()) {
        let mut epoch_guard = self.epoch.lock().unwrap();
        *epoch_guard += 1;
        f();
        drop(epoch_guard);
        self.cvar.notify_all();
    }

    fn next_after_timeout(&self, f: impl FnOnce()) {
        let mut epoch_guard = self.epoch.lock().unwrap();
        let old_epoch = *epoch_guard;
        let deadline = Instant::now() + MAX_TIME_BETWEEN_BATCHES;
        loop {
            let now = Instant::now();
            let wait_duration = deadline.checked_duration_since(now).unwrap_or_default();

            let result = self.cvar.wait_timeout(epoch_guard, wait_duration).unwrap();
            epoch_guard = result.0;
            if old_epoch == *epoch_guard {
                // execute only after timeout (check spurious wakeups)
                if result.1.timed_out() {
                    *epoch_guard += 1;
                    f();
                    break;
                }
            } else {
                break;
            }
        }
    }
}
