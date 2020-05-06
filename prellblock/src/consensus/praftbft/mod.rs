//! `PRaftBFT` is a consensus algorithm.
//! Hopefully it is fast. We don't know.
//! Such Intro
//! Much Information
//!
//! [Benchmark Results](https://www.youtube.com/watch?v=dQw4w9WgXcQ)

#![allow(clippy::mutex_atomic)]

mod error;
mod follower;
mod leader;
pub mod message;
mod ring_buffer;
mod state;
mod view_change;

pub use error::Error;

use crate::{
    block_storage::BlockStorage,
    consensus::LeaderTerm,
    peer::{message as peer_message, Sender},
    permission_checker::PermissionChecker,
    world_state::WorldStateService,
    BoxError,
};
use futures::{stream::FuturesUnordered, StreamExt};
use im::Vector;
use leader::Leader;
use message::ConsensusMessage;
use pinxit::{Identity, PeerId, Signable, Signature, Signed};
use prellblock_client_api::Transaction;
use state::{FollowerState, LeaderState};
use std::{
    collections::{HashMap, VecDeque},
    iter::FromIterator,
    net::SocketAddr,
    ops::Deref,
    sync::Arc,
    time::Instant,
};
use tokio::sync::{watch, Mutex, Notify, RwLock};

const MAX_TRANSACTIONS_PER_BLOCK: usize = 4000;

type ViewChangeSignatures = HashMap<PeerId, Signature>;

type Queue = VecDeque<(Instant, Signed<Transaction>)>;
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
    queue: Arc<RwLock<Queue>>,
    leader_notifier: Arc<Notify>,
    follower_state: Mutex<FollowerState>,
    world_state: WorldStateService,
    // For unblocking waiting out-of-order messages.
    block_changed_notifier: watch::Sender<()>,
    block_changed_receiver: watch::Receiver<()>,
    broadcast_meta: BroadcastMeta,
    new_view_sender: watch::Sender<LeaderTerm>,
    new_view_receiver: watch::Receiver<LeaderTerm>,
    enough_view_changes_sender: watch::Sender<(LeaderTerm, ViewChangeSignatures)>,
    block_storage: BlockStorage,
    permission_checker: PermissionChecker,
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
        block_storage: BlockStorage,
        world_state: WorldStateService,
    ) -> Arc<Self> {
        log::debug!("Started consensus.");

        assert!(
            world_state
                .get()
                .peers
                .iter()
                .any(|(peer_id, _)| identity.id() == peer_id),
            "The identity is not part of the peers list."
        );

        let broadcast_meta = BroadcastMeta {
            identity,
            world_state: world_state.clone(),
        };

        let follower_state = FollowerState::from_world_state(&world_state.get());
        let leader_state = LeaderState::new(&follower_state);
        let leader_term = follower_state.leader_term;
        let follower_state = Mutex::new(follower_state);
        let leader_notifier = Arc::new(Notify::new());
        let (block_changed_notifier, block_changed_receiver) = watch::channel(());
        let (new_view_sender, new_view_receiver) = watch::channel(leader_term);
        let (enough_view_changes_sender, enough_view_changes_receiver) =
            watch::channel((leader_term, ViewChangeSignatures::new()));
        let praftbft = Self {
            queue: Arc::default(),
            leader_notifier: leader_notifier.clone(),
            follower_state,
            world_state: world_state.clone(),
            block_changed_notifier,
            block_changed_receiver,
            broadcast_meta,
            new_view_sender,
            new_view_receiver,
            enough_view_changes_sender,
            block_storage,
            permission_checker: PermissionChecker::new(world_state),
        };

        let praftbft = Arc::new(praftbft);

        let leader = Leader {
            praftbft: praftbft.clone(),
            leader_state,
        };

        tokio::spawn(leader.process_transactions(leader_notifier, enough_view_changes_receiver));
        {
            let praftbft_clone = praftbft.clone();
            tokio::spawn(async move {
                praftbft_clone
                    .censorship_checker(praftbft_clone.new_view_receiver.clone())
                    .await;
            });
        }
        praftbft
    }

    /// Stores incoming `Transaction`s in the Consensus' `queue`.
    pub async fn take_transactions(&self, transactions: Vec<Signed<Transaction>>) {
        // Add timestamp and block number at time of arrival for censorship protection.
        let new_entries = transactions
            .into_iter()
            .map(|transaction| (Instant::now(), transaction));
        let mut new_entries = VecDeque::from_iter(new_entries);
        let mut queue = self.queue.write().await;
        queue.append(&mut new_entries);
        if queue.len() >= MAX_TRANSACTIONS_PER_BLOCK {
            drop(queue);
            self.leader_notifier.notify();
        }
    }

    /// Checks whether a number represents f + 1 nodes
    fn nonfaulty_reached(&self, number: usize) -> bool {
        let majority = (self.peers_len() - 1) / 3 + 1;
        number >= majority
    }
}

#[derive(Clone)]
pub struct BroadcastMeta {
    identity: Identity,
    world_state: WorldStateService,
}

impl BroadcastMeta {
    /// Retrieve the consesus' own identity's id.
    const fn peer_id(&self) -> &PeerId {
        self.identity.id()
    }

    fn peers(&self) -> Vector<(PeerId, SocketAddr)> {
        self.world_state.get().peers
    }

    /// The number of peers in the consensus.
    fn peers_len(&self) -> usize {
        self.peers().len()
    }

    fn peer_ids(&self) -> impl Iterator<Item = PeerId> {
        self.peers().into_iter().map(|(peer_id, _)| peer_id)
    }

    fn is_current_leader(&self, leader_term: LeaderTerm, peer_id: &PeerId) -> bool {
        peer_id.clone() == self.leader(leader_term)
    }

    fn leader(&self, leader_term: LeaderTerm) -> PeerId {
        let index = u64::from(leader_term) % (self.peers_len() as u64);
        #[allow(clippy::cast_possible_truncation)]
        self.peers()[index as usize].0.clone()
    }

    async fn broadcast_until_majority<F>(
        &self,
        message: ConsensusMessage,
        verify_response: F,
    ) -> Result<HashMap<PeerId, Signature>, BoxError>
    where
        F: Fn(&ConsensusMessage) -> Result<(), BoxError> + Clone + Send + Sync + 'static,
    {
        let message = message.sign(&self.identity)?;
        let signed_message = peer_message::Consensus(message);

        let mut futures = FuturesUnordered::new();

        for (_, peer_address) in self.peers() {
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
                    Ok(response) => Some(response),
                    Err(err) => {
                        log::warn!("Consensus error from {}: {}", peer_address, err);
                        None
                    }
                }
            }));
        }

        let mut responses = HashMap::new();

        while let Some(result) = futures.next().await {
            match result {
                Ok(Some((peer_id, signature))) => {
                    responses.insert(peer_id, signature);
                }
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
        let peer_count = self.peers().len();
        if peer_count < 4 {
            panic!("Cannot find consensus for less than four peers.");
        }
        let supermajority = peer_count * 2 / 3 + 1;
        response_len >= supermajority
    }
}
