#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! Library Crate used for Communication between external Clients and internal RPUs.

pub mod account;
pub mod consensus;

use account::Account;
use balise::define_api;
use consensus::{Block, BlockNumber};
use pinxit::{PeerId, Signable, Signature, Signed};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ops::{Bound, Deref, RangeBounds},
    time::{Duration, SystemTime},
};

/// Play ping pong. See [`Ping`](message/struct.Ping.html).
#[derive(Debug, Serialize, Deserialize)]
pub struct Pong;

/// Filter to select a value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Filter<T> {
    /// Select one exactly matching value.
    Exact(T),
    /// Select a range of values.
    Range(std::ops::Range<T>),
    /// Select an unbound range of values, starting from a given value.
    RangeFrom(T),
}

#[allow(clippy::use_self)]
impl<T> Filter<T> {
    /// Converts from `Filter<T>` (or `&Filter<T>`) to `Filter<&T::Target>`.
    ///
    /// Leaves the original Filter in-place, creating a new one with a reference
    /// to the original one, additionally coercing the contents via `Deref`.
    pub fn as_deref(&self) -> Filter<&T::Target>
    where
        T: Deref,
    {
        match self {
            Self::Exact(value) => Filter::Exact(&**value),
            Self::Range(range) => Filter::Range(&*range.start..&*range.end),
            Self::RangeFrom(value) => Filter::RangeFrom(&**value),
        }
    }
}

#[allow(clippy::match_same_arms)]
impl<T> RangeBounds<T> for Filter<T> {
    fn start_bound(&self) -> Bound<&T> {
        match self {
            Self::Exact(value) => Bound::Included(value),
            Self::Range(range) => range.start_bound(),
            Self::RangeFrom(value) => Bound::Included(value),
        }
    }

    fn end_bound(&self) -> Bound<&T> {
        match self {
            Self::Exact(value) => Bound::Included(value),
            Self::Range(range) => range.end_bound(),
            Self::RangeFrom(_) => Bound::Unbounded,
        }
    }
}

/// A span or selection of a given point in time / number of values.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum Span {
    /// A number of values.
    Count(usize),
    /// A point in time.
    Time(SystemTime),
    /// A duration of time.
    Duration(Duration),
}

/// A `Query` describribes what kind of value will be retreived.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Query {
    /// Get the current value.
    CurrentValue,
    /// Get all value of a `Peer`.
    AllValues,
    /// Get all values selected by `Span`s.
    Range {
        /// The span to fetch.
        span: Span,
        /// The last value to fetch.
        ///
        /// `Count` and `Duration` are relative to the last value.
        end: Span,
        /// An optional specification to skip some values.
        ///
        /// `Time` does not make sense in this context and will be ignored.
        skip: Option<Span>,
    },
}

/// The `Transaction`s in response to a `GetValue` request of a single data series of a `Peer`.
pub type ReadTransactionsOfSeries = HashMap<SystemTime, (Vec<u8>, Signature)>;

/// The `Transaction`s in response to a `GetValue` request of a single `Peer`.
pub type ReadTransactionsOfPeer = HashMap<String, ReadTransactionsOfSeries>;

/// The `Transaction`s in response to a `GetValue` request of all `Peer`s.
pub type ReadTransactions = HashMap<PeerId, ReadTransactionsOfPeer>;

define_api! {
    /// The message API module for communication between RPUs.
    mod message;
    /// One of the requests.
    pub enum ClientMessage {
        /// Ping Message. See [`Pong`](../struct.Pong.html).
        Ping => Pong,

        /// Simple transaction Message. Will write a key:value pair.
        Execute(Signed<Transaction>) => (),

        /// Get the values of the given peers, filtered by a filter and selected by a query.
        GetValue(Vec<PeerId>, Filter<String>, Query) => ReadTransactions,

        /// Get a single account by it's `PeerId`.
        GetAccount(Vec<PeerId>) => Vec<Account>,

        /// Get a `Block` by it's `BlockNumber`.
        GetBlock(Filter<BlockNumber>) => Vec<Block>,

        /// Get the current number of blocks in the blockchain.
        GetCurrentBlockNumber() => BlockNumber,
    }
}

/// A blockchain transaction for prellblock.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Transaction {
    /// Set a `key` to a `value`.
    KeyValue {
        /// The key.
        key: String,
        /// The value.
        value: Vec<u8>,
    },
}

impl Signable for Transaction {
    type SignableData = Vec<u8>;
    type Error = postcard::Error;
    fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
        postcard::to_stdvec(self)
    }
}
