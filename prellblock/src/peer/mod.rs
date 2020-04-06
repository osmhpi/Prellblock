//! Message types that can be used to communicate between RPUs.
//!
//! # Example
//!
//! ```
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

#![allow(clippy::wildcard_imports)]

mod calculator;
pub mod message;
mod receiver;
mod sender;

pub use calculator::Calculator;
use message::*;
pub use receiver::Receiver;
pub use sender::Sender;

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// One of the requests.
#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
pub enum RequestData {
    Add(Add),
    Sub(Sub),
    Ping(Ping),
}
