//! Message types that can be used to communicate between RPUs.
//!
//! # Example
//!
//! ```no_run
//! use prellblock::{
//!     data_storage::DataStorage,
//!     peer::{message, Calculator, PeerInbox, Receiver, Sender},
//! };
//! use std::{net::TcpListener, sync::Arc};
//!
//! // start a receiver
//! let calculator = Calculator::new();
//! let calculator = Arc::new(calculator.into());
//!
//! let data_storage = DataStorage::new("/tmp/some_db").unwrap(); // don't use tmp
//! let data_storage = Arc::new(data_storage);
//!
//! let peer_inbox = PeerInbox::new(calculator, data_storage);
//! let peer_inbox = Arc::new(peer_inbox);
//!
//! let bind_addr = "127.0.0.1:0"; // replace 0 with a useful port
//! let listener = TcpListener::bind(bind_addr).unwrap();
//! let peer_addr = listener.local_addr().unwrap(); // address with allocated port
//!
//! std::thread::spawn(move || {
//!     let receiver = Receiver::new("path_to_pfx.pfx".to_string(), peer_inbox);
//!     receiver.serve(&listener).unwrap();
//! });
//!
//! // use a sender to execute some requests
//! let mut sender = Sender::new(peer_addr);
//! match sender.send_request(message::Ping) {
//!     Err(err) => log::error!("Failed to send Ping: {}", err),
//!     Ok(res) => log::debug!("Ping response: {:?}", res),
//! }
//! ```

mod calculator;
mod peer_inbox;
mod receiver;
mod sender;

pub use calculator::Calculator;
pub use peer_inbox::PeerInbox;
pub use receiver::Receiver;
pub use sender::Sender;

use balise::define_api;
use pinxit::{PeerId, Signed};
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

        /// Simple transaction message. Will write a key:value pair.
        Execute(PeerId, Signed<Transaction>) => (),

        /// Simple transaction message. Will write a key:value pair.
        ExecuteBatch(Vec<Execute>) => (),
    }
}
