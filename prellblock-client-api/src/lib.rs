#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! Library Crate used for Communication between external Clients and internal RPUs.

pub mod account_permissions;

use account_permissions::{Expiry, ReadingRight};
use balise::define_api;
use newtype_enum::newtype_enum;
use pinxit::{PeerId, Signable, Signed};
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
        Execute(Signed<Transaction>) => (),
    }
}

/// A blockchain transaction for prellblock.
#[newtype_enum(variants = "pub transaction")]
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Transaction {
    /// Set a `key` to a `value`.
    KeyValue {
        /// The key.
        key: String,

        /// The value.
        value: Vec<u8>,
    },

    /// Update an account.
    UpdateAccount {
        /// The account to set the permissions for.
        id: PeerId,

        /// Setting this to `true` enables accounts to take part in the the consensus.
        is_admin: Option<bool>,

        /// Setting this to `true` enables accounts to take part in the the consensus.
        is_rpu: Option<bool>,

        /// Setting this will make an account expire at the given timestamp.
        expire_at: Option<Expiry>,

        /// Setting this to `true` enables an accounts to write data to its own namespace.
        has_writing_rights: Option<bool>,

        /// Permissions for reading from other accounts.
        reading_rights: Option<Vec<ReadingRight>>,
    },
}

impl Signable for Transaction {
    type SignableData = Vec<u8>;
    type Error = postcard::Error;
    fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
        postcard::to_stdvec(self)
    }
}
