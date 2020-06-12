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
///
/// // ranges can be constructed via some `Into` implementations:
/// # use std::fmt::Debug;
/// fn assert_into<T: Eq + Debug>(a: Filter<T>, b: impl Into<Filter<T>>) {
///     assert_eq!(a, b.into());
/// }
///
/// assert_into(Filter::Exact(42), 42);
/// assert_into(Filter::Range(0..42), 0..42);
/// assert_into(Filter::RangeFrom(42), 42..);
///
/// // Querying all values *only works with strings*.
/// assert_into(Filter::RangeFrom(String::new()), ..);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Filter<T> {
    /// Select one exactly matching value.
    Exact(T),
    /// Select a range of values.
    Range(std::ops::Range<T>),
    /// Select an unbound range of values, starting from a given value.
    RangeFrom(T),
}

impl<T> From<T> for Filter<T> {
    fn from(v: T) -> Self {
        Self::Exact(v)
    }
}

impl<T> From<std::ops::Range<T>> for Filter<T> {
    fn from(v: std::ops::Range<T>) -> Self {
        Self::Range(v)
    }
}

impl From<std::ops::RangeFull> for Filter<String> {
    fn from(_: std::ops::RangeFull) -> Self {
        Self::RangeFrom(String::new())
    }
}

impl<T> From<std::ops::RangeFrom<T>> for Filter<T> {
    fn from(v: std::ops::RangeFrom<T>) -> Self {
        Self::RangeFrom(v.start)
    }
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

impl From<usize> for Span {
    fn from(v: usize) -> Self {
        Self::Count(v)
    }
}
impl From<SystemTime> for Span {
    fn from(v: SystemTime) -> Self {
        Self::Time(v)
    }
}
impl From<Duration> for Span {
    fn from(v: Duration) -> Self {
        Self::Duration(v)
    }
}

///
/// # Examples
/// ```
/// use prellblock_client_api::{Query, Span};
/// use std::time::{Duration, SystemTime};
///
/// fn timestamp(s: &str) -> SystemTime {
///     chrono::DateTime::parse_from_rfc3339(s).unwrap().into()
/// }
///
/// let timestamp_8_am = timestamp("2020-05-22T08:00:00Z");
/// let timestamp_10_am = timestamp("2020-05-22T10:00:00Z");
///
/// // fetch the last 1000 values
/// Query::Range {
///     span: 1000.into(),
///     end: 0.into(),
///     skip: None,
/// };
///
/// // fetch every other value 1000 times going backward from the current value.
/// Query::Range {
///     span: 1000.into(),
///     end: 0.into(),
///     skip: Some(1.into()),
/// };
///
/// // fetch all values between T-60 and T-20.
/// Query::Range {
///     span: Duration::from_secs(40 * 60).into(),
///     end: Duration::from_secs(20 * 60).into(),
///     skip: None,
/// };
///
/// // fetch all new values after 10 AM.
/// Query::Range {
///     span: timestamp_10_am.into(),
///     end: 0.into(),
///     skip: None,
/// };
///
/// // fetch all values between 8 AM and 10 AM.
/// Query::Range {
///     span: timestamp_8_am.into(),
///     end: timestamp_10_am.into(),
///     skip: None,
/// };
///
/// // fetch a value every minute between 8 AM and 10 AM.
/// Query::Range {
///     span: timestamp_8_am.into(),
///     end: timestamp_10_am.into(),
///     skip: Some(Duration::from_secs(60).into()),
/// };
///
/// // fetch 100 values before 8 AM with 5 minutes intervals.
/// Query::Range {
///     span: 100.into(),
///     end: timestamp_8_am.into(),
///     skip: Some(Duration::from_secs(5 * 60).into()),
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
pub type ReadValuesOfSeries = HashMap<SystemTime, (Vec<u8>, SystemTime, Signature)>;

/// The `Transaction`s in response to a `GetValue` request of a single peer.
pub type ReadValuesOfPeer = HashMap<String, ReadValuesOfSeries>;

/// The `Transaction`s in response to a `GetValue` request of all peers.
pub type ReadValues = HashMap<PeerId, ReadValuesOfPeer>;

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
        GetValue(Signed<crate::GetValue>) => ReadValues,

        /// Get a single account by it's `PeerId`.
        ///
        /// Accounts that are not found will be omitted in the return value.
        GetAccount(Signed<crate::GetAccount>) => Vec<Account>,

        /// Get a `Block` by it's `BlockNumber`.
        GetBlock(Signed<crate::GetBlock>) => Vec<Block>,

        /// Get the current number of blocks in the blockchain.
        GetCurrentBlockNumber(Signed<crate::GetCurrentBlockNumber>) => BlockNumber,
    }
}

/// Get the values of the given peers, filtered by a filter and selected by a query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetValue {
    /// A Vector of `PeerId`'s to select the `Accounts` from which to read.
    pub peer_ids: Vec<PeerId>,
    /// The filter to select some keys of the namespace.
    pub filter: Filter<String>,
    /// The query to selct some values in the given time range.
    pub query: Query,
}

/// Get a single account by it's `PeerId`.
///
/// Accounts that are not found will be omitted in the return value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAccount {
    /// A Vector of `PeerId`'s to select the `Accounts` from which to read.
    pub peer_ids: Vec<PeerId>,
}

/// Get a `Block` by it's `BlockNumber`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBlock {
    /// The filter to select some blocks.
    pub filter: Filter<BlockNumber>,
}

/// Get the current number of blocks in the blockchain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCurrentBlockNumber;

#[derive(Serialize)]
enum ClientMessageSigningData<'a> {
    Execute(&'a Transaction),
    GetValue(&'a GetValue),
    GetAccount(&'a GetAccount),
    GetBlock(&'a GetBlock),
    GetCurrentBlockNumber(&'a GetCurrentBlockNumber),
}

macro_rules! impl_signable {
    ($($ident:ident => $ty:ty),*) => {$(
        impl Signable for $ty {
            type SignableData = Vec<u8>;
            type Error = postcard::Error;
            fn signable_data(&self) -> Result<Self::SignableData, Self::Error> {
                postcard::to_stdvec(&ClientMessageSigningData::$ident(self))
            }
        }
    )*};
}

impl_signable!(
    Execute => Transaction,
    GetValue => GetValue,
    GetAccount => GetAccount,
    GetBlock => GetBlock,
    GetCurrentBlockNumber => GetCurrentBlockNumber
);

/// A blockchain transaction for prellblock.
#[allow(clippy::large_enum_variant)]
#[newtype_enum(variants = "transaction")]
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Transaction {
    /// Set a `key` to a `value`.
    KeyValue {
        /// The key.
        key: String,
        /// The value.
        value: Vec<u8>,
        /// The Timestamp.
        timestamp: SystemTime,
    },
    /// Update an account.
    UpdateAccount {
        /// The account to set the permissions for.
        id: PeerId,
        /// The permission fields to update.
        permissions: Permissions,
        /// The Timestamp.
        timestamp: SystemTime,
    },
    /// Create an account.
    CreateAccount {
        /// An ID for the new account.
        id: PeerId,
        /// The name for the new account.
        name: String,
        /// The permission fields to set.
        permissions: Permissions,
        /// The timestamp of transaction creation.
        timestamp: SystemTime,
    },
    /// Delete an account.
    DeleteAccount {
        /// The account to delete.
        id: PeerId,
        /// The timestamp of transaction creation.
        timestamp: SystemTime,
    },
}

/// A trait signifying that a transaction can be written into the Account-tree in the `DataStorage`.
pub trait AccountTransaction {
    /// The timestamp of transaction creation.
    fn timestamp(&self) -> SystemTime;
}

impl AccountTransaction for transaction::UpdateAccount {
    fn timestamp(&self) -> SystemTime {
        self.timestamp
    }
}
impl AccountTransaction for transaction::CreateAccount {
    fn timestamp(&self) -> SystemTime {
        self.timestamp
    }
}
impl AccountTransaction for transaction::DeleteAccount {
    fn timestamp(&self) -> SystemTime {
        self.timestamp
    }
}
