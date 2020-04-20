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
mod state;

pub use error::Error;

use flatten_vec::FlattenVec;
use pinxit::{Identity, PeerId, Signed};
use prellblock_client_api::Transaction;
use state::{FollowerState, LeaderState};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{mpsc, Arc, Mutex},
    thread,
};

const MAX_TRANSACTIONS_PER_BLOCK: usize = 25;

type Waker = mpsc::SyncSender<()>;
type Sleeper = mpsc::Receiver<()>;

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
    queue: Mutex<FlattenVec<Signed<Transaction>>>,
    follower_state: Mutex<FollowerState>,
    leader_state: Option<Mutex<LeaderState>>,
    peers: HashMap<PeerId, SocketAddr>,
    /// Our own identity, used for signing messages.
    identity: Identity,
    /// Trigger processing of transactions.
    waker: Waker,
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

        let (waker, sleeper) = mpsc::sync_channel(0);

        // TODO: Remove this.
        let leader_id =
            PeerId::from_hex("b7f85ce58ff74f34f6600891082af745bcc70df35cca49e816bdeca96924ce99")
                .unwrap();

        let leader_state = if *identity.id() == leader_id {
            Some(Mutex::default())
        } else {
            None
        };

        let praftbft = Self {
            queue: Mutex::default(),
            follower_state: Mutex::default(),
            leader_state,
            identity,
            peers,
            waker,
        };

        // TODO: Remove this.
        {
            let mut follower_state = praftbft.follower_state.lock().unwrap();
            follower_state.leader = Some(leader_id);
        }

        let praftbft = Arc::new(praftbft);
        {
            let praftbft = praftbft.clone();
            thread::spawn(move || praftbft.process_transactions(&sleeper));
        }
        praftbft
    }

    /// Stores incoming `Transaction`s in the Consensus' `queue`.
    pub fn take_transactions(&self, transactions: Vec<Signed<Transaction>>) {
        let mut queue = self.queue.lock().unwrap();

        queue.push(transactions);

        if queue.len() >= MAX_TRANSACTIONS_PER_BLOCK {
            // TODO: Restart thread for processing messages.
            self.waker
                .send(())
                .expect("Processing thread is not running.");
        }
    }
}
