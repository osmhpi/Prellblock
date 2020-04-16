//! `PRaftBFT` is a consensus algorithm.
//! Hopefully it is fast. We don't know.
//! Such Intro
//! Much Information
//!
//! [Benchmark Results](https://www.youtube.com/watch?v=dQw4w9WgXcQ)

mod error;
mod flatten_vec;
pub mod message;

pub use error::Error;

use super::{Block, BlockHash, Body};
use crate::{
    peer::{message as peer_message, Sender, SignedTransaction},
    thread_group::ThreadGroup,
    BoxError,
};
use flatten_vec::FlattenVec;
use message::ConsensusMessage;
use pinxit::{Identity, PeerId, Signable, Signature, Signed};
use prellblock_client_api::Transaction;
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
    queue: Mutex<FlattenVec<SignedTransaction>>,
    state: Mutex<State>,
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

        let praftbft = Self {
            queue: Mutex::default(),
            state: Mutex::default(),
            identity,
            peers,
            waker,
        };
        let praftbft = Arc::new(praftbft);
        {
            let praftbft = praftbft.clone();
            thread::spawn(move || praftbft.process_transactions(sleeper));
        }
        praftbft
    }

    /// This function waits until it is triggered to process transactions.
    fn process_transactions(&self, sleeper: Sleeper) {
        loop {
            // TODO: sleep until timeout
            sleeper.recv().expect(
                "The consensus died. Stopping processing transaction in background thread.",
            );

            // TODO: use > 0 instead, when in timeout
            while self.queue.lock().unwrap().len() >= MAX_TRANSACTIONS_PER_BLOCK {
                let mut transactions = Vec::with_capacity(MAX_TRANSACTIONS_PER_BLOCK);

                // TODO: Check size of transactions cumulated.
                while let Some(transaction) = self.queue.lock().unwrap().next() {
                    // pack block
                    // TODO: Validate transaction.

                    transactions.push(transaction);

                    if transactions.len() >= MAX_TRANSACTIONS_PER_BLOCK {
                        break;
                    }
                }

                let body = Body {
                    block_num: 1,
                    prev_block_hash: BlockHash::default(),
                    transactions,
                };
                let hash = body.hash();
                let mut state = self.state.lock().unwrap();
                // do prepare
                let prepare_message = ConsensusMessage::Prepare {
                    leader_term: 21,
                    sequence_number: state.sequence + 1,
                    block_hash: hash,
                };
                self.broadcast_until_majority(prepare_message, |response| unimplemented!());

                // do append

                // do commit
            }
        }
    }

    fn broadcast_until_majority<F>(
        &self,
        message: ConsensusMessage,
        verify_response: F,
    ) -> Result<(), BoxError>
    where
        F: Fn(&ConsensusMessage) -> Result<(), BoxError> + Clone + Send + 'static,
    {
        let own_id = self.identity.id().clone();
        let message = message.sign(&self.identity)?;
        let signed_message = peer_message::Consensus(own_id, message);

        let mut thread_group = ThreadGroup::new();
        let (tx, rx) = mpsc::sync_channel(0);

        for (_, &peer_address) in &self.peers {
            let signed_message = signed_message.clone();
            let verify_response = verify_response.clone();
            let tx = tx.clone();
            thread_group.spawn(
                &format!("Send consensus message to {}", peer_address),
                move || {
                    let send_message_and_verify_response = || {
                        let mut sender = Sender::new(peer_address);
                        let (peer_id, response) = sender.send_request(signed_message)?;
                        let verified_response = response.verify(&peer_id)?;
                        verify_response(&*verified_response)?;
                        Ok::<_, BoxError>((peer_id, verified_response.signature().clone()))
                    };

                    tx.send(send_message_and_verify_response()).unwrap();
                },
            );
        }

        let mut responses = Vec::new();
        while !self.supermajority_reached(responses.len()) {
            match rx.recv() {
                Ok(Ok((peer_id, signature))) => responses.push((peer_id, signature)),
                Ok(Err(err)) => {
                    log::warn!("The consensus f*d up: {}", err);
                }
                Err(err) => {
                    log::warn!("The re group f*d up: {}", err);
                }
            };
        }

        // TODO: once async io is used, drop the unused threads

        Ok(())
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

    /// Stores incoming `Transaction`s in the Consensus' `queue`.
    pub fn take_transactions(&self, transactions: Vec<SignedTransaction>) {
        let mut queue = self.queue.lock().unwrap();

        queue.push(transactions);

        if queue.len() >= MAX_TRANSACTIONS_PER_BLOCK {
            // TODO: Restart thread for processing messages.
            self.waker
                .send(())
                .expect("Processing thread is not running.");
        }
    }

    fn handle_prepare_message(
        &self,
        peer_id: PeerId,
        leader_term: usize,
        sequence_number: usize,
        block_hash: BlockHash,
    ) -> Result<ConsensusMessage, Error> {
        let mut state = self.state.lock().unwrap();
        state.verify_message_meta(&peer_id, leader_term)?;

        // Only process new messages.
        if sequence_number <= state.sequence {
            log::warn!("Current Leader's Sequence number is too small.");
            return Err(Error::SequenceNumberTooSmall);
        }

        // All checks passed, update our state.
        state.current_block_hash = block_hash;
        state.sequence = sequence_number;

        // Send AckPrepare to the leader.
        let ackprepare_message = ConsensusMessage::AckPrepare {
            leader_term: state.leader_term,
            sequence_number: state.sequence,
            peer_id: self.identity.id().clone(),
            block_hash: state.current_block_hash.clone(),
        };

        // Done :D
        Ok(ackprepare_message)
    }

    fn handle_append_message(
        &self,
        peer_id: PeerId,
        leader_term: usize,
        sequence_number: usize,
        block_hash: BlockHash,
        ackprepare_signatures: Vec<(PeerId, Signature)>,
        data: Vec<Signed<Transaction>>,
    ) -> Result<ConsensusMessage, Error> {
        let state = self.state.lock().unwrap();
        state.verify_message_meta(&peer_id, leader_term)?;
        // Only process messages of the same sequence.
        if sequence_number != state.sequence {
            log::warn!("Current Leader's Sequence number is incorrect.");
            return Err(Error::WrongSequenceNumber);
        }

        // TODO: Remove this.
        let _ = (block_hash, ackprepare_signatures, data);
        unimplemented!();
    }

    /// Process the incoming `ConsensusMessages` (`PREPARE`, `ACKPREPARE`, `APPEND`, `ACKAPPEND`, `COMMIT`)
    pub fn handle_message(
        &self,
        peer_id: PeerId,
        message: Signed<ConsensusMessage>,
    ) -> Result<(PeerId, Signed<ConsensusMessage>), Error> {
        // Only RPUs are allowed.
        if !self.peers.contains_key(&peer_id) {
            return Err(Error::InvalidPeer(peer_id));
        }

        let message = message.verify(&peer_id)?;

        let response = match message.into_inner() {
            ConsensusMessage::Prepare {
                leader_term,
                sequence_number,
                block_hash,
            } => self.handle_prepare_message(peer_id, leader_term, sequence_number, block_hash)?,
            ConsensusMessage::Append {
                leader_term,
                sequence_number,
                block_hash,
                ackprepare_signatures,
                data,
            } => self.handle_append_message(
                peer_id,
                leader_term,
                sequence_number,
                block_hash,
                ackprepare_signatures,
                data,
            )?,
            _ => unimplemented!(),
        };

        let signed_response = response.sign(&self.identity).unwrap();
        Ok((self.identity.id().clone(), signed_response))
    }
}

#[derive(Default)]
struct State {
    leader_term: usize,
    sequence: usize,
    current_block_hash: BlockHash,
    leader: Option<PeerId>,
}

impl State {
    /// Validate that there is a leader and the received message is from this leader.
    fn verify_message_meta(&self, peer_id: &PeerId, leader_term: usize) -> Result<(), Error> {
        // We only handle the current leader term.
        if leader_term != self.leader_term {
            log::warn!("Follower is not in the correct Leader term");
            return Err(Error::WrongLeaderTerm);
        }

        // There should be a known leader.
        let leader = if let Some(leader) = &self.leader {
            leader
        } else {
            // TODO: Trigger leader fetch or election?
            log::warn!("No current leader set");
            return Err(Error::NoLeader);
        };

        // Leader must be the same as we know for the current leader term.
        if leader != peer_id {
            log::warn!(
                "Received Prepare message from invalid leader (ID: {}).",
                peer_id
            );
            return Err(Error::WrongLeader(peer_id.clone()));
        }

        Ok(())
    }
}
