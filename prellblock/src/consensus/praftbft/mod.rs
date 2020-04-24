//! `PRaftBFT` is a consensus algorithm.
//! Hopefully it is fast. We don't know.
//! Such Intro
//! Much Information
//!
//! [Benchmark Results](https://www.youtube.com/watch?v=dQw4w9WgXcQ)

#![allow(clippy::mutex_atomic)]

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
    sync::{mpsc, Arc, Condvar, Mutex},
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
    broadcast_meta: BroadcastMeta,
    view_change_cvar: Arc<(Mutex<usize>, Condvar)>,
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

        let broadcast_meta = BroadcastMeta { peers, identity };
        let (waker, sleeper) = mpsc::sync_channel(0);

        let follower_state = FollowerState::new();
        let leader_state = Mutex::new(LeaderState::new(&follower_state));
        let leader_term = follower_state.leader_term;
        let follower_state = Mutex::new(follower_state);
        let praftbft = Self {
            queue: Mutex::default(),
            follower_state,
            leader_state,
            broadcast_meta,
            view_change_cvar: Arc::new((Mutex::new(leader_term), Condvar::new())),
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
        let majority = (self.peers_len() - 1) / 3 + 1;
        number >= majority
    }

    /// Check whether a number represents a supermajority (>2/3) compared
    /// to the peers in the consenus.
    fn supermajority_reached(&self, number: usize) -> bool {
        supermajority_reached(number, self.peers_len())
    }

    /// Retrieve the consesus' own identity's id.
    const fn peer_id(&self) -> &PeerId {
        self.broadcast_meta.identity.id()
    }

    /// The number of peers in the consensus.
    fn peers_len(&self) -> usize {
        self.broadcast_meta.peers.len()
    }

    fn peer_ids(&self) -> impl Iterator<Item = &PeerId> {
        self.broadcast_meta.peers.iter().map(|(peer_id, _)| peer_id)
    }

    fn broadcast_until_majority<F>(
        &self,
        message: ConsensusMessage,
        verify_response: F,
    ) -> Result<Vec<(PeerId, Signature)>, BoxError>
    where
        F: Fn(&ConsensusMessage) -> Result<(), BoxError> + Clone + Send + 'static,
    {
        self.broadcast_meta
            .broadcast_until_majority(message, verify_response)
    }

    fn is_current_leader(&self, leader_term: usize, peer_id: &PeerId) -> bool {
        peer_id.clone() == self.leader(leader_term).clone()
    }

    fn leader(&self, leader_term: usize) -> &PeerId {
        &self.broadcast_meta.peers[leader_term % self.peers_len()].0
    }
}

/// Check whether a number represents a supermajority (>2/3) compared
/// to the total number of peers (`peer_count`) in the consenus.
pub(super) fn supermajority_reached(response_len: usize, peer_count: usize) -> bool {
    if peer_count < 4 {
        panic!("Cannot find consensus for less than four peers.");
    }
    let supermajority = peer_count * 2 / 3 + 1;
    response_len >= supermajority
}

#[derive(Clone)]
struct BroadcastMeta {
    peers: Vec<(PeerId, SocketAddr)>,
    identity: Identity,
}

impl BroadcastMeta {
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
            if supermajority_reached(responses.len(), self.peers.len()) {
                // TODO: once async io is used, drop the unused threads
                return Ok(responses);
            }
        }

        // All sender threads have died **before reaching supermajority**.
        Err("Could not get supermajority.".into())
    }
}
