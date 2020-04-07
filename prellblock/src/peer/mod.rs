//! Message types that can be used to communicate between RPUs.
//!
//! # Example
//!
//! ```no_run
//! use prellblock::peer::{message, Calculator, Receiver, Sender};
//! use std::{net::TcpListener, sync::Arc};
//!
//! // start a receiver
//! let calculator = Calculator::new();
//! let calculator = Arc::new(calculator.into());
//!
//! let bind_addr = "127.0.0.1:0"; // replace 0 with a useful port
//! let listener = TcpListener::bind(bind_addr).unwrap();
//! let peer_addr = listener.local_addr().unwrap(); // address with allocated port
//!
//! std::thread::spawn(move || {
//!     let receiver = Receiver::new(calculator, "path_to_pfx.pfx".to_string());
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

// TODO: Define and use a macro to create the api spec boiler plate. (macro_rules!)
// See https://doc.rust-lang.org/book/ch19-06-macros.html
// and https://rustbyexample.com/macros.html

mod calculator;
mod receiver;
mod sender;

pub use calculator::Calculator;
pub use receiver::Receiver;
pub use sender::Sender;

use balise::define_api;
use pinxit::{PeerId, Signature};
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

        /// Simple transaction Message. Will write a key:value pair.
        SetValue(PeerId, String,serde_json::Value, Signature) => (),
    }
}
