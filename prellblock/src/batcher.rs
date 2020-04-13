//! Module used for batching messages for a `Broadcaster`.

#![allow(clippy::mutex_atomic)]

use crate::{data_broadcaster::Broadcaster, peer::message};
use std::{
    mem,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

const MAX_TRANSACTIONS_PER_BATCH: usize = 100;
const MAX_TIME_BETWEEN_BATCHES: Duration = Duration::from_secs(1);

/// A Batcher for messages.
pub struct Batcher {
    bucket: Mutex<Bucket>,
    epoch: Epoch,
}

struct Bucket {
    broadcaster: Arc<Broadcaster>,
    bucket: Vec<message::Execute>,
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
    pub fn add_to_batch(self: Arc<Self>, message: message::Execute) {
        let mut bucket = self.bucket.lock().unwrap();
        let shared_self = self.clone();
        if bucket.bucket.is_empty() {
            thread::spawn(move || {
                shared_self.epoch.next_after_timeout(|| {
                    shared_self.bucket.lock().unwrap().send_to_broadcaster()
                });
            });
        }
        bucket.bucket.push(message);
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
        loop {
            let result = self
                .cvar
                .wait_timeout(epoch_guard, MAX_TIME_BETWEEN_BATCHES)
                .unwrap();
            epoch_guard = result.0;
            if old_epoch == *epoch_guard {
                *epoch_guard += 1;
                f();
                break;
            }
        }
    }
}
