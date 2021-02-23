mod state;

use super::{
    message::{consensus_message as message, consensus_response as response},
    Core, Error, RingBuffer,
};
use crate::consensus::{BlockNumber, LeaderTerm};
use pinxit::{PeerId, Signature};
use state::State;
use std::{
    future::Future,
    ops::Deref,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{sync::Notify, time};

const NEW_VIEW_TIMEOUT: Duration = Duration::from_millis(1000);
const RING_BUFFER_SIZE: usize = 64;

#[derive(Debug)]
pub struct ViewChange {
    core: Arc<Core>,
    notify_new_view: Notify,
    state: Mutex<State>,
}

impl Deref for ViewChange {
    type Target = Core;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl ViewChange {
    pub fn new(core: Arc<Core>) -> Self {
        Self {
            core,
            notify_new_view: Notify::new(),
            state: Mutex::new(State::new(RING_BUFFER_SIZE)),
        }
    }

    /// Get the `NewView` message if one is available for the leader.
    pub fn get_new_view_message(
        &self,
        current_block_number: BlockNumber,
    ) -> Option<message::NewView> {
        let mut state = self.state.lock().unwrap();

        let leader_term = state.leader_term;
        if self.leader(leader_term) == *self.identity.id() {
            state
                .current_signatures
                .take()
                .map(|view_change_signatures| message::NewView {
                    leader_term,
                    current_block_number,
                    view_change_signatures,
                })
        } else {
            None
        }
    }

    /// Send a `ConsensusMessage::ViewChange` if an error occurs
    /// (the leader seems to be faulty).
    #[allow(clippy::future_not_send)]
    pub async fn request_view_change_on_error<T>(
        &self,
        future: impl Future<Output = Result<T, Error>>,
    ) -> Result<T, Error> {
        match future.await {
            Ok(value) => Ok(value),
            Err(err) => {
                self.request_view_change().await;
                Err(err)
            }
        }
    }

    /// Send a `ConsensusMessage::ViewChange` message because the leader
    /// seems to be faulty.
    pub async fn request_view_change(&self) {
        let new_leader_term = self.state.lock().unwrap().leader_term + 1;
        self.request_view_change_in_leader_term(new_leader_term)
            .await;
    }

    /// Send a `ConsensusMessage::ViewChange` for a given `leader_term`
    /// because the leader seems to be faulty.
    pub async fn request_view_change_in_leader_term(&self, new_leader_term: LeaderTerm) {
        // No need to update the state,
        // we broadcast the message also to ourselves.

        self.broadcast_view_change(new_leader_term).await;
    }

    /// Broadcast a `ViewChange` message for a `new_leader_term`.
    async fn broadcast_view_change(&self, new_leader_term: LeaderTerm) {
        log::trace!("Broadcasting ViewChange Message: {}", new_leader_term);

        let message = message::ViewChange { new_leader_term };
        match self.broadcast_until_majority(message, |_| Ok(())).await {
            Ok(_) => log::info!(
                "ViewChange Message Broadcast {} did reach supermajority.",
                new_leader_term
            ),
            Err(err) => log::warn!(
                "ViewChange Message Broadcast {} did not reach supermajority: {}",
                new_leader_term,
                err
            ),
        };
    }

    /// Handle a `ViewChange` message.
    pub fn handle_view_change(
        self: &Arc<Self>,
        peer_id: PeerId,
        signature: Signature,
        new_leader_term: LeaderTerm,
    ) -> Result<response::Ok, Error> {
        let mut state = self.state.lock().unwrap();

        let signatures = state.future_signatures.get_mut(new_leader_term)?;

        if signatures.insert(peer_id, signature).is_some() {
            // Ignore duplicate signature
            return Ok(response::Ok);
        }

        if signatures.len() == self.nonfaulty_count() {
            // if enough collected, broadcast message and update state accordingly

            let cloned_self = self.clone();
            tokio::spawn(async move {
                cloned_self.broadcast_view_change(new_leader_term).await;
            });
        }

        if self.supermajority_reached(signatures.len()) {
            state.did_reach_supermajority(new_leader_term);

            // Notify leader task to begin to work.
            self.notify_leader.notify_one();

            // Start the new view timeout.
            drop(state);
            self.notify_new_view.notify_one();
        }

        Ok(response::Ok)
    }

    /// A `NewView` message arrived for a given `leader_term`.
    pub fn new_view_received(&self, leader_term: LeaderTerm) {
        let mut state = self.state.lock().unwrap();
        if state.leader_term == leader_term {
            state.new_view_time = None;

            // The new view arrived in time.
            drop(state);
            self.notify_new_view.notify_one();
        }
    }

    /// A taks that handles `NewView` timeouts.
    pub async fn new_view_timeout_checker(self: Arc<Self>) {
        loop {
            match self.new_view_duration() {
                // Check if the `NewView` message arrives in time.
                Some(new_view_duration) => self.check_new_view_timeout(new_view_duration).await,
                // Wait for the newxt `NewView` message timeout
                None => self.notify_new_view.notified().await,
            }
        }
    }

    /// Check if the `NewView` message arrives in time
    /// after `new_view_duration` has already passed.
    async fn check_new_view_timeout(&self, new_view_duration: Duration) {
        let new_view_time_left = NEW_VIEW_TIMEOUT.checked_sub(new_view_duration);

        let new_view_arrived_in_time = if let Some(remaining_time) = new_view_time_left {
            time::timeout(remaining_time, self.notify_new_view.notified())
                .await
                .is_ok()
        } else {
            // timeout already reached
            false
        };

        if new_view_arrived_in_time {
            log::trace!("NewView arrived in time.");
        } else {
            log::debug!("NewView has not arrived in time.");
            self.request_view_change().await;
        }
    }

    /// Returns the duration since the `NewView` timeout started
    /// or `None` if the `NewView` message arrived in time.
    fn new_view_duration(&self) -> Option<Duration> {
        self.state
            .lock()
            .unwrap()
            .new_view_time
            .as_ref()
            .map(Instant::elapsed)
    }

    /// Calculates the number that represents f + 1 nodes.
    fn nonfaulty_count(&self) -> usize {
        (self.world_state.get().peers.len() - 1) / 3 + 1
    }
}
