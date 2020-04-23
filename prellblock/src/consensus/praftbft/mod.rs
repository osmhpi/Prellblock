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
mod view_change;

pub use error::Error;

use crate::block_storage::BlockStorage;
use flatten_vec::FlattenVec;
use message::ConsensusMessage;
use pinxit::{Identity, PeerId, Signable, Signature, Signed};
use prellblock_client_api::Transaction;
use state::{FollowerState, LeaderState};
use std::{
    net::SocketAddr,
    sync::{mpsc, Arc, Mutex},
    thread,
};

use crate::{
    peer::{message as peer_message, Sender},
    thread_group::ThreadGroup,
    BoxError,
};

const MAX_TRANSACTIONS_PER_BLOCK: usize = 5;

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
    leader_state: Mutex<LeaderState>,
    peers: Vec<(PeerId, SocketAddr)>,
    /// Our own identity, used for signing messages.
    identity: Identity,
    block_storage: BlockStorage,
    /// Trigger processing of transactions.
    waker: Waker,
}

impl PRaftBFT {
    /// Create new `PRaftBFT` Instance.
    ///
    /// The instance is identified `identity` and in a group with other `peers`.
    /// **Warning:** This starts a new thread for processing transactions in the background.
    #[must_use]
    pub fn new(
        identity: Identity,
        peers: Vec<(PeerId, SocketAddr)>,
        block_storage: BlockStorage,
    ) -> Arc<Self> {
        log::debug!("Started consensus with peers: {:?}", peers);
        let mut exists = false;
        for (peer_id, _) in peers.clone() {
            if identity.id() == &peer_id {
                exists = true;
            }
        }
        assert!(exists, "The identity is not part of the peers list.");
        let (waker, sleeper) = mpsc::sync_channel(0);

        let follower_state = FollowerState::new();
        let leader_state = Mutex::new(LeaderState::new(&follower_state));
        let follower_state = Mutex::new(follower_state);
        let praftbft = Self {
            queue: Mutex::default(),
            follower_state,
            leader_state,
            identity,
            peers,
            block_storage,
            waker,
        };

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
            drop(queue);
            // TODO: Restart thread for processing messages.
            self.waker
                .send(())
                .expect("Processing thread is not running.");
        }
    }

    /// Checks whether a number represents f + 1 nodes
    fn nonfaulty_reached(&self, number: usize) -> bool {
        let majority = (self.peers.len() - 1) / 3 + 1;
        number >= majority
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

    fn broadcast_until_majority<F>(
        &self,
        message: ConsensusMessage,
        verify_response: F,
    ) -> Result<Vec<(PeerId, Signature)>, BoxError>
    where
        F: Fn(&ConsensusMessage) -> Result<(), BoxError> + Clone + Send + 'static,
    {
        let message = message.sign(&self.identity)?;
        let signed_message = peer_message::Consensus(message);

        let mut thread_group = ThreadGroup::new();
        let (tx, rx) = mpsc::sync_channel(0);

        for (_, peer_address) in self.peers.clone() {
            let signed_message = signed_message.clone();
            let verify_response = verify_response.clone();
            let tx = tx.clone();
            thread_group.spawn(
                &format!("Send consensus message to {}", peer_address),
                move || {
                    let send_message_and_verify_response = || {
                        let mut sender = Sender::new(peer_address);
                        let response = sender.send_request(signed_message)?;
                        let signer = response.signer().clone();
                        let verified_response = response.verify()?;
                        verify_response(&*verified_response)?;
                        Ok::<_, BoxError>((signer, verified_response.signature().clone()))
                    };

                    // The rx-side is closed when we probably collected enough signatures.
                    let _ = tx.send(send_message_and_verify_response());
                },
            );
        }

        // IMPORTANT: when we do not drop this tx, the loop below will loop forever
        drop(tx);

        let mut responses = Vec::new();

        for result in rx {
            match result {
                Ok((peer_id, signature)) => responses.push((peer_id, signature)),
                Err(err) => {
                    log::warn!("Consensus Error: {}", err);
                }
            }
            if self.supermajority_reached(responses.len()) {
                // TODO: once async io is used, drop the unused threads
                return Ok(responses);
            }
        }

        // All sender threads have died **before reaching supermajority**.
        Err("Could not get supermajority.".into())
    }

    fn is_current_leader(&self, leader_term: usize, peer_id: &PeerId) -> bool {
        peer_id.clone() == self.leader(leader_term).clone()
    }

    fn leader(&self, leader_term: usize) -> &PeerId {
        &self.peers[leader_term % self.peers.len()].0
    }
}
