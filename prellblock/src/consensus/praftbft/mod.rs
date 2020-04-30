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

use crate::{
    block_storage::BlockStorage,
    peer::{message as peer_message, Sender},
    world_state::WorldStateService,
    BoxError,
};
use flatten_vec::FlattenVec;
use futures::{stream::FuturesUnordered, StreamExt};
use leader::Leader;
use message::ConsensusMessage;
use pinxit::{Identity, PeerId, Signable, Signature, Signed};
use prellblock_client_api::Transaction;
use state::{FollowerState, LeaderState};
use std::{net::SocketAddr, ops::Deref, sync::Arc};
use tokio::sync::{watch, Mutex, Notify};

const MAX_TRANSACTIONS_PER_BLOCK: usize = 4000;

type ViewChangeSignatures = Vec<(PeerId, Signature)>;

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
    world_state: WorldStateService,
    // For unblocking waiting out-of-order messages.
    sequence_changed_notifier: watch::Sender<()>,
    sequence_changed_receiver: watch::Receiver<()>,
    broadcast_meta: BroadcastMeta,
    view_change_sender: watch::Sender<usize>,
    view_change_receiver: watch::Receiver<usize>,
    leader_term_sender: watch::Sender<(usize, ViewChangeSignatures)>,
    block_storage: BlockStorage,
}

impl Deref for PRaftBFT {
    type Target = BroadcastMeta;
    fn deref(&self) -> &Self::Target {
        &self.broadcast_meta
    }
}

impl PRaftBFT {
    /// Create new `PRaftBFT` Instance.
    ///
    /// The instance is identified `identity` and in a group with other `peers`.
    /// **Warning:** This starts a new thread for processing transactions in the background.
    pub async fn new(
        identity: Identity,
        peers: Vec<(PeerId, SocketAddr)>,
        block_storage: BlockStorage,
        world_state: WorldStateService,
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

        let follower_state = FollowerState::from_world_state(&world_state.get());
        let leader_state = LeaderState::new(&follower_state);
        let leader_term = follower_state.leader_term;
        let follower_state = Mutex::new(follower_state);
        let leader_notifier = Arc::new(Notify::new());
        let (sequence_changed_notifier, sequence_changed_receiver) = watch::channel(());
        let (view_change_sender, view_change_receiver) = watch::channel(leader_term);
        let (leader_term_sender, leader_term_receiver) =
            watch::channel((leader_term, ViewChangeSignatures::new()));
        let praftbft = Self {
            queue: Arc::default(),
            leader_notifier: leader_notifier.clone(),
            follower_state,
            world_state,
            sequence_changed_notifier,
            sequence_changed_receiver,
            broadcast_meta,
            view_change_sender,
            view_change_receiver,
            leader_term_sender,
            block_storage,
        };

        let praftbft = Arc::new(praftbft);

        let leader = Leader {
            praftbft: praftbft.clone(),
            leader_state,
        };

        tokio::spawn(leader.process_transactions(leader_notifier, leader_term_receiver));

        praftbft
    }

    /// Stores incoming `Transaction`s in the Consensus' `queue`.
    pub async fn take_transactions(&self, transactions: Vec<Signed<Transaction>>) {
        let mut queue = self.queue.lock().await;
        queue.push(transactions);
        self.leader_notifier.notify();
    }

    /// Checks whether a number represents f + 1 nodes
    fn nonfaulty_reached(&self, number: usize) -> bool {
        let majority = (self.peers_len() - 1) / 3 + 1;
        number >= majority
    }
}

#[derive(Clone)]
pub struct BroadcastMeta {
    peers: Vec<(PeerId, SocketAddr)>,
    identity: Identity,
}

impl BroadcastMeta {
    /// Retrieve the consesus' own identity's id.
    const fn peer_id(&self) -> &PeerId {
        self.identity.id()
    }

    /// The number of peers in the consensus.
    fn peers_len(&self) -> usize {
        self.peers.len()
    }

    fn peer_ids(&self) -> impl Iterator<Item = &PeerId> {
        self.peers.iter().map(|(peer_id, _)| peer_id)
    }

    fn is_current_leader(&self, leader_term: usize, peer_id: &PeerId) -> bool {
        peer_id.clone() == self.leader(leader_term).clone()
    }

    fn leader(&self, leader_term: usize) -> &PeerId {
        &self.peers[leader_term % self.peers_len()].0
    }

    async fn broadcast_until_majority<F>(
        &self,
        message: ConsensusMessage,
        verify_response: F,
    ) -> Result<Vec<(PeerId, Signature)>, BoxError>
    where
        F: Fn(&ConsensusMessage) -> Result<(), BoxError> + Clone + Send + Sync + 'static,
    {
        let message = message.sign(&self.identity)?;
        let signed_message = peer_message::Consensus(message);

        let mut futures = FuturesUnordered::new();

        for &(_, peer_address) in &self.peers {
            let signed_message = signed_message.clone();
            let verify_response = verify_response.clone();

            futures.push(tokio::spawn(async move {
                let send_message_and_verify_response = async {
                    let mut sender = Sender::new(peer_address);
                    let response = sender.send_request(signed_message).await?;
                    let signer = response.signer().clone();
                    let verified_response = response.verify()?;
                    verify_response(&*verified_response)?;
                    Ok::<_, BoxError>((signer, verified_response.signature().clone()))
                };
                // TODO: Are seperate threads (tokio::spawn) faster?
                match send_message_and_verify_response.await {
                    Ok((peer_id, signature)) => Some((peer_id, signature)),
                    Err(err) => {
                        log::warn!("Consensus error from {}: {}", peer_address, err);
                        None
                    }
                }
            }));
        }

        let mut responses = Vec::new();

        while let Some(result) = futures.next().await {
            match result {
                Ok(Some(response)) => responses.push(response),
                Ok(None) => {}
                Err(err) => log::warn!("Failed to join task: {}", err),
            }
            if self.supermajority_reached(responses.len()) {
                return Ok(responses);
            }
        }

        // All sender tasks have died **before reaching supermajority**.
        Err("Could not get supermajority.".into())
    }

    /// Check whether a number represents a supermajority (>2/3) compared
    /// to the total number of peers (`peer_count`) in the consenus.
    fn supermajority_reached(&self, response_len: usize) -> bool {
        let peer_count = self.peers.len();
        if peer_count < 4 {
            panic!("Cannot find consensus for less than four peers.");
        }
        let supermajority = peer_count * 2 / 3 + 1;
        response_len >= supermajority
    }
}
