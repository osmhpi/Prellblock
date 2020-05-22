#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! Library Crate used for Communication between external Clients and internal RPUs.

pub mod account;
pub mod consensus;

use account::{Account, Permissions};
use balise::define_api;
use consensus::{Block, BlockNumber};
use newtype_enum::newtype_enum;
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
///
/// # Examples
/// ```
/// use prellblock_client_api::Filter;
///
/// // fetch the items identified by 42
/// Filter::Exact(42);
///
/// // fetch the items identified by the values between 0 (inclusive) and 42 (exclusive)
/// Filter::Range(0..42);
///
/// // fetch the items identified by the values starting from 42 (inclusive)
/// Filter::RangeFrom(42);
///
/// // fetch the items identified by the value prefix "temperature" (between "temperature" (inclusive) and "temperaturf" (exclusive))
/// Filter::Range("temperature".."temperaturf");
///
/// // fetch the items identified by the values starting from "temperature" (inclusive)
/// Filter::RangeFrom("temperature");
/// ```
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

/// A `Query` describes what kind of value will be retreived.
///
/// # Examples
/// ```
/// use chrono::{DateTime, Utc};
/// use prellblock_client_api::{Query, Span};
/// use std::time::Duration;
///
/// let timestamp_8_am = DateTime::parse_from_rfc3339("2020-05-22T08:00:00Z").unwrap();
/// let timestamp_10_am = DateTime::parse_from_rfc3339("2020-05-22T10:00:00Z").unwrap();
///
/// // fetch the last 1000 values
/// Query::Range {
///     span: Span::Count(1000),
///     end: Span::Count(0),
///     skip: None,
/// };
///
/// // fetch every other value 1000 times going backward from the current value.
/// Query::Range {
///     span: Span::Count(1000),
///     end: Span::Count(0),
///     skip: Some(Span::Count(1)),
/// };
///
/// // fetch all values between T-60 and T-20.
/// Query::Range {
///     span: Span::Duration(Duration::from_secs(40 * 60)),
///     end: Span::Duration(Duration::from_secs(20 * 60)),
///     skip: None,
/// };
///
/// // fetch all new values after 10 AM.
/// Query::Range {
///     span: Span::Time(timestamp_10_am.into()),
///     end: Span::Count(0),
///     skip: None,
/// };
///
/// // fetch all values between 8 AM and 10 AM.
/// Query::Range {
///     span: Span::Time(timestamp_8_am.into()),
///     end: Span::Time(timestamp_10_am.into()),
///     skip: None,
/// };
///
/// // fetch a value every minute between 8 AM and 10 AM.
/// Query::Range {
///     span: Span::Time(timestamp_8_am.into()),
///     end: Span::Time(timestamp_10_am.into()),
///     skip: Some(Span::Duration(Duration::from_secs(60))),
/// };
///
/// // fetch 100 values before 8 AM with 5 minutes intervals.
/// Query::Range {
///     span: Span::Count(100),
///     end: Span::Time(timestamp_8_am.into()),
///     skip: Some(Span::Duration(Duration::from_secs(5 * 60))),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Query {
    /// Get the current value.
    CurrentValue,
    /// Get all value of a peer.
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

/// The `Transaction`s in response to a `GetValue` request of a single data series of a peer.
pub type ReadTransactionsOfSeries = HashMap<SystemTime, (Vec<u8>, Signature)>;

/// The `Transaction`s in response to a `GetValue` request of a single peer.
pub type ReadTransactionsOfPeer = HashMap<String, ReadTransactionsOfSeries>;

/// The `Transaction`s in response to a `GetValue` request of all peers.
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
        ///
        /// Accounts that are not found will be omitted in the return value.
        GetAccount(Vec<PeerId>) => Vec<Account>,

        /// Get a `Block` by it's `BlockNumber`.
        GetBlock(Filter<BlockNumber>) => Vec<Block>,

        /// Get the current number of blocks in the blockchain.
        GetCurrentBlockNumber() => BlockNumber,
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
        /// The permission fields to update.
        permissions: Permissions,
    },
}

impl Signable for Transaction {
    type SignableData = Vec<u8>;
    type Error = postcard::Error;
    fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
        postcard::to_stdvec(self)
    }
}
