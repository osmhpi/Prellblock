//! The `BlockStorage` is a permantent storage for validated Blocks persisted on disk.

mod error;

pub use error::Error;

use crate::{
    consensus::{Block, BlockHash, BlockNumber},
    transaction_checker::AccountChecker,
};
use pinxit::{PeerId, Signature};
use prellblock_client_api::{
    Filter, Query, ReadValuesOfPeer, ReadValuesOfSeries, Span, Transaction,
};
use sled::{Config, Db, Tree};
use std::{
    collections::HashMap,
    convert::TryInto,
    fmt::Debug,
    mem,
    ops::{Bound, RangeBounds},
    str,
    time::{Duration, SystemTime},
};

const BLOCKS_TREE_NAME: &[u8] = b"blocks";
const ACCOUNTS_TREE_NAME: &[u8] = b"accounts";

/// A `BlockStorage` provides persistent storage on disk.
///
/// Data is written to disk every 400ms.
#[derive(Debug, Clone)]
pub struct BlockStorage {
    database: Db,
    blocks: Tree,
    accounts: Tree,
}

impl BlockStorage {
    /// Create a new `Store` at path.
    pub fn new(path: &str) -> Result<Self, Error> {
        let config = Config::default()
            .path(path)
            .cache_capacity(8_000_000)
            .flush_every_ms(Some(400))
            .snapshot_after_ops(100)
            .use_compression(false) // TODO: set this to `true`.
            .compression_factor(20);

        let database = config.open()?;
        let blocks = database.open_tree(BLOCKS_TREE_NAME)?;
        let accounts = database.open_tree(ACCOUNTS_TREE_NAME)?;

        Ok(Self {
            database,
            blocks,
            accounts,
        })
    }

    /// Write a value to the store.
    ///
    /// The data will be accessible by the block number?.
    pub fn write_block(&self, block: &Block) -> Result<(), Error> {
        let (last_block_hash, block_number) = if let Some(last_block) = self.read(..).next_back() {
            let last_block = last_block?;
            (last_block.hash(), last_block.body.height + 1)
        } else {
            (BlockHash::default(), BlockNumber::default())
        };

        if block.body.prev_block_hash != last_block_hash {
            return Err(Error::BlockHashDoesNotMatch);
        }

        if block.body.height != block_number {
            return Err(Error::BlockHeightDoesNotFit);
        }

        let value = postcard::to_stdvec(&block)?;
        self.blocks
            .insert(block.block_number().to_be_bytes(), value)?;

        for transaction in &block.body.transactions {
            match transaction.unverified_ref() {
                Transaction::KeyValue(params) => {
                    self.write_value(
                        transaction.signer(),
                        &params.key,
                        &params.value,
                        &params.timestamp,
                        transaction.signature(),
                    )?;
                }
                // We don't need to do anything here. Account permissions are saved in the `WorldState`.
                Transaction::UpdateAccount(_) => {}
            }
        }

        Ok(())
    }

    /// Write the peer's id to the peer tree.
    /// Write the key to the timeseries tree of the peer.
    /// Write the transaction to the general transaction tree.
    fn write_value(
        &self,
        peer_id: &PeerId,
        key: &str,
        value: &[u8],
        timestamp: &SystemTime,
        signature: &Signature,
    ) -> Result<(), Error> {
        // Add the peer to the account db.
        self.accounts.insert(peer_id.as_bytes(), &[])?;

        // Add the value name the time_series tree.
        self.database
            .open_tree(peer_id.as_bytes())?
            .insert(key, &[])?;

        // Insert value with timestamp of receival and the client's timestamp into the time_series tree.
        let time_series_name = [peer_id.as_bytes(), key.as_bytes()].join(&0);
        let write_time = SystemTime::now();

        // Write time has to be the first one because it is used when reading.
        let time = system_times_to_bytes(write_time, *timestamp);
        let data = postcard::to_stdvec(&(value, signature))?;
        self.database
            .open_tree(time_series_name)?
            .insert(time, data)?;

        Ok(())
    }

    /// Read a range of blocks from the store.
    pub fn read<R>(&self, range: R) -> impl DoubleEndedIterator<Item = Result<Block, Error>>
    where
        R: RangeBounds<BlockNumber> + Debug + Clone,
    {
        let range_string = if log::log_enabled!(log::Level::Trace) {
            format!("{:?}", range)
        } else {
            String::new()
        };
        self.blocks
            .range(map_range_bound(range, |v| v.to_be_bytes()))
            .values()
            .map(move |result| {
                let value = result?;
                let block = postcard::from_bytes(&value)?;
                log::trace!("Read block from range {}: {:#?}", range_string, block);
                Ok(block)
            })
    }

    /// Read transactions filtered by a `Filter` and a `Query` from `Blockstorage`.
    pub fn read_transactions(
        &self,
        account_checker: &AccountChecker,
        peer_id: &PeerId,
        filter: Filter<&str>,
        query: &Query,
    ) -> Result<ReadValuesOfPeer, Error> {
        self.database
            .open_tree(peer_id.as_bytes())?
            .range(filter)
            .keys()
            .filter_map(|key| {
                let inner = || {
                    let key = key?;
                    let key = str::from_utf8(&key).unwrap();
                    if !account_checker.is_allowed_to_read_key(peer_id, key) {
                        return Ok(None);
                    }
                    let time_series_name = [peer_id.as_bytes(), key.as_bytes()].join(&0);
                    let transactions = self.read_transactions_inner(&time_series_name, query)?;
                    let key = key.into();
                    Ok(Some((key, transactions)))
                };
                inner().transpose()
            })
            .collect()
    }

    /// Get a all transactions of a `time_series`, filtered by a `Query`, in a `HashMap`.
    fn read_transactions_inner(
        &self,
        time_series_name: &[u8],
        query: &Query,
    ) -> Result<ReadValuesOfSeries, Error> {
        let mut transactions = HashMap::new();

        match query {
            // Get the latest value in this series.
            Query::CurrentValue => {
                if let Some((key, value)) = self
                    .read_time_series(time_series_name, ..)?
                    .rev()
                    .next()
                    .transpose()?
                {
                    transactions.insert(key, value);
                }
            }
            // Get all values in this series.
            Query::AllValues => {
                for result in self.read_time_series(time_series_name, ..)? {
                    let (key, value) = result?;
                    transactions.insert(key, value);
                }
            }
            // Get all values of a give `Range`.
            Query::Range { span, end, skip } => {
                let mut skip_end = 0;
                let end = match *end {
                    Span::Count(count) => {
                        skip_end = count;
                        Bound::Unbounded
                    }
                    Span::Time(time) => Bound::Excluded(time),
                    Span::Duration(duration) => Bound::Excluded(SystemTime::now() - duration),
                };

                let mut iter = self
                    .read_time_series(time_series_name, ((Bound::Unbounded), end))?
                    .rev()
                    .peekable();

                // Skip to last wanted value
                for _ in 0..skip_end {
                    iter.next().transpose()?;
                }

                let mut span = *span;
                while let Some((key, value)) = iter.next().transpose()? {
                    match &mut span {
                        Span::Count(count) => {
                            if *count == 0 {
                                break;
                            } else {
                                *count -= 1;
                            }
                        }
                        Span::Time(time) => {
                            if key < *time {
                                break;
                            }
                        }
                        Span::Duration(duration) => span = Span::Time(key - *duration),
                    }
                    transactions.insert(key, value);
                    // Skip items according to `skip`
                    if let Some(skip) = skip {
                        match skip {
                            Span::Count(count) => {
                                for _ in 0..*count {
                                    iter.next().transpose()?;
                                }
                            }
                            Span::Time(_) => {}
                            Span::Duration(duration) => {
                                let skip_to = key - *duration;
                                while let Some(Ok((key, _))) = iter.peek() {
                                    if *key < skip_to {
                                        break;
                                    }
                                    iter.next().transpose()?;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(transactions)
    }

    // Read a timeseries from `BlockStorage` and transform the raw data into a `Transaction` tuple.
    // The first timestamp is the time, the value was stored on the RPU.
    // The second one is the timestamp given by the client.
    fn read_time_series<R>(
        &self,
        time_series_name: &[u8],
        range: R,
    ) -> Result<
        impl DoubleEndedIterator<Item = Result<(SystemTime, (Vec<u8>, SystemTime, Signature)), Error>>,
        Error,
    >
    where
        R: RangeBounds<SystemTime>,
    {
        let iter = self
            .database
            .open_tree(time_series_name)?
            .range(map_range_bound(range, |v| {
                system_times_to_bytes(*v, SystemTime::UNIX_EPOCH)
            })) // RPU write time
            .map(|result| {
                let (key, value) = result?;
                let key = system_times_from_bytes(&key);
                let value: (Vec<u8>, Signature) = postcard::from_bytes(&value)?;
                Ok((key.0, (value.0, key.1, value.1)))
            });
        Ok(iter)
    }

    /// Remove the last block (at the end of the chain) and return it.
    pub fn pop_block(&self) -> Result<Option<Block>, Error> {
        if let Some((_, value)) = self.blocks.pop_max()? {
            let block: Block = postcard::from_bytes(&value)?;

            // update value tree
            for transaction in &block.body.transactions {
                match transaction.unverified_ref() {
                    Transaction::KeyValue(params) => {
                        let peer_id = transaction.signer();
                        let time_series_name = [peer_id.as_bytes(), params.key.as_bytes()].join(&0);
                        self.database.open_tree(time_series_name)?.pop_max()?;
                    }
                    // We don't need to do anything here. Account permissions are rolled back in the `WorldState`.
                    Transaction::UpdateAccount(_) => {}
                }
            }

            Ok(Some(block))
        } else {
            Ok(None)
        }
    }
}

fn map_range_bound<T, R, U>(range_bound: R, mut f: impl FnMut(&T) -> U) -> impl RangeBounds<U>
where
    R: RangeBounds<T>,
{
    (
        map_bound(range_bound.start_bound(), |v| f(v)),
        map_bound(range_bound.end_bound(), |v| f(v)),
    )
}

fn map_bound<T, U>(bound: Bound<T>, f: impl FnOnce(T) -> U) -> Bound<U> {
    match bound {
        Bound::Included(v) => Bound::Included(f(v)),
        Bound::Excluded(v) => Bound::Excluded(f(v)),
        Bound::Unbounded => Bound::Unbounded,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn system_times_to_bytes(first: SystemTime, second: SystemTime) -> impl AsRef<[u8]> {
    let first = match first.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as i64,
        Err(err) => -(err.duration().as_nanos() as i64),
    }
    .to_be_bytes();
    let second = match second.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos() as i64,
        Err(err) => -(err.duration().as_nanos() as i64),
    }
    .to_be_bytes();
    [first, second].concat()
}

#[allow(clippy::cast_sign_loss)]
fn system_times_from_bytes(bytes: &[u8]) -> (SystemTime, SystemTime) {
    if bytes.len() != 2 * mem::size_of::<i64>() {
        panic!("Invalid timestamp key length. Could not read two timestamps.");
    }

    let (first, second) = bytes.split_at(mem::size_of::<i64>());
    let first = i64::from_be_bytes(first.try_into().unwrap());
    let first = if first >= 0 {
        let duration = Duration::from_nanos(first as u64);
        SystemTime::UNIX_EPOCH + duration
    } else {
        let duration = Duration::from_nanos((-(first + 1)) as u64 + 1);
        SystemTime::UNIX_EPOCH - duration
    };

    let second = i64::from_be_bytes(second.try_into().unwrap());
    let second = if second >= 0 {
        let duration = Duration::from_nanos(second as u64);
        SystemTime::UNIX_EPOCH + duration
    } else {
        let duration = Duration::from_nanos((-(second + 1)) as u64 + 1);
        SystemTime::UNIX_EPOCH - duration
    };

    (first, second)
}
