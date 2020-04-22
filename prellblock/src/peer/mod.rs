//! Message types that can be used to communicate between RPUs.

mod calculator;
mod peer_inbox;
mod receiver;
mod sender;

pub use calculator::Calculator;
pub use peer_inbox::PeerInbox;
pub use receiver::Receiver;
pub use sender::Sender;

use crate::consensus::ConsensusMessage;
use balise::define_api;
use pinxit::Signed;
use prellblock_client_api::Transaction;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Play ping pong. See [`Ping`](message/struct.Ping.html).
#[derive(Debug, Serialize, Deserialize)]
pub struct Pong;

define_api! {
    /// The message API module for communication between RPUs.
    mod message;
    /// One of the requests.
    pub enum PeerMessage {
        /// Add two numbers.
        Add(usize, usize) => usize,

        /// Subtract two numbers.
        Sub(usize, usize) => usize,

        /// Ping Message. See [`Pong`](../struct.Pong.html).
        Ping => Pong,

        /// Simple batch of transaction message. Will write a key:value pair.
        ExecuteBatch(Vec<Signed<Transaction>>) => (),

        /// Messages exchanged by the consensus.
        Consensus(Signed<ConsensusMessage>) => Signed<ConsensusMessage>,
    }
}
