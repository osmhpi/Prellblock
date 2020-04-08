#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! Library Crate used for Communication between external Clients and internal RPUs.

use balise::define_api;
use pinxit::{PeerId, Signable, Signature};
use serde::{Deserialize, Serialize};

/// Play ping pong. See [`Ping`](message/struct.Ping.html).
#[derive(Debug, Serialize, Deserialize)]
pub struct Pong;

define_api! {
    /// The message API module for communication between RPUs.
    mod message;
    /// One of the requests.
    pub enum ClientMessage {
        /// Ping Message. See [`Pong`](../struct.Pong.html).
        Ping => Pong,
        /// Simple transaction Message. Will write a key:value pair.
        SetValue(PeerId, String,serde_json::Value, Signature) => (),
    }
}

/// The transaction message for key:value creation.
#[derive(Debug, Serialize)]
pub struct TransactionMessage<'a> {
    /// The key.
    pub key: &'a str,
    /// The value.
    pub value: &'a serde_json::Value,
}

impl<'a> Signable for TransactionMessage<'a> {
    type Message = String;
    type Error = serde_json::error::Error;
    fn message(&self) -> Result<Self::Message, Self::Error> {
        serde_json::to_string(self)
    }
}
