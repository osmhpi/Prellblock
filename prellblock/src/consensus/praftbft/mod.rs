//! `PRaftBFT` is a consensus algorithm.
//! Hopefully it is fast. We don't know.
//! Such Intro
//! Much Information
//!
//! [Benchmark Results](https://www.youtube.com/watch?v=dQw4w9WgXcQ)

mod error;
mod flatten_vec;
mod follower;
mod leader;
pub mod message;
mod ring_buffer;
mod state;

pub use error::Error;

use flatten_vec::FlattenVec;
use leader::Leader;
use pinxit::{Identity, PeerId, Signed};
use prellblock_client_api::Transaction;
use state::{FollowerState, LeaderState};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Condvar, Mutex},
};
use tokio::sync::Notify;

const MAX_TRANSACTIONS_PER_BLOCK: usize = 1;

/// Prellblock Raft BFT consensus algorithm.
///
/// See the [paper](https://www.scs.stanford.edu/17au-cs244b/labs/projects/clow_jiang.pdf).
pub struct PRaftBFT {
    // Was muss der können?

    // - Peer Inbox -> Transaktionen entgegennehmen (und im RAM behalten)
    // - Ordering betreiben
    // - Transaktionen sammeln bis Trigger zum Block vorschlagen
    // - Nachrichten über Peer Sender senden
    // - Nachrichten von PeerInbox empfangen
    // - fertige Blöcke übergeben an prellblock
    queue: Arc<Mutex<FlattenVec<Signed<Transaction>>>>,
    leader_notifier: Arc<Notify>,
    follower_state: Mutex<FollowerState>,
    follower_state_sequence_changed: Condvar,
    peers: HashMap<PeerId, SocketAddr>,
    /// Our own identity, used for signing messages.
    identity: Identity,
}

impl PRaftBFT {
    /// Create new `PRaftBFT` Instance.
    ///
    /// The instance is identified `identity` and in a group with other `peers`.
    /// **Warning:** This starts a new thread for processing transactions in the background.
    #[must_use]
    pub fn new(identity: Identity, peers: HashMap<PeerId, SocketAddr>) -> Arc<Self> {
        log::debug!("Started consensus with peers: {:?}", peers);
        assert!(
            peers.get(identity.id()).is_some(),
            "The identity is not part of the peers list."
        );

        // TODO: Remove this.
        let leader_id =
            PeerId::from_hex("c72d59b472b5f511a88f3f1f8804c498d891ffd91583fec9f23541b9fefd3585")
                .unwrap();

        let leader = Leader {
            identity: identity.clone(),
            queue: Arc::default(),
            peers: peers.clone(),
            leader_state: LeaderState::default(),
        };
        let queue = leader.queue.clone();

        let leader_notifier = Arc::new(Notify::new());
        if identity.id() == &leader_id {
            tokio::spawn(leader.process_transactions(leader_notifier.clone()));
        }

        let praftbft = Self {
            queue,
            leader_notifier,
            follower_state: Mutex::new(FollowerState::new()),
            follower_state_sequence_changed: Condvar::default(),
            peers,
            identity,
        };

        // TODO: Remove this.
        {
            let mut follower_state = praftbft.follower_state.lock().unwrap();
            follower_state.leader = Some(leader_id);
        }

        Arc::new(praftbft)
    }

    /// Stores incoming `Transaction`s in the Consensus' `queue`.
    pub fn take_transactions(&self, transactions: Vec<Signed<Transaction>>) {
        let mut queue = self.queue.lock().unwrap();
        queue.push(transactions);
        self.leader_notifier.notify();
    }

    /// Check whether a number represents a supermajority (>2/3) compared
    /// to the peers in the consenus.
    fn supermajority_reached(&self, number: usize) -> bool {
        let len = self.peers.len();
        if len < 4 {
            panic!("Cannot find consensus for less than four peers.");
        }
        let supermajority = len * 2 / 3 + 1;
        number >= supermajority
    }
}
