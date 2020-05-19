//! The `BlockStorage` is a permantent storage for validated Blocks persisted on disk.

mod error;

pub use error::Error;

use crate::consensus::{Block, BlockHash, BlockNumber};
use pinxit::PeerId;
use prellblock_client_api::Transaction;
use sled::{Config, Db, Tree};
use std::ops::{Bound, RangeBounds};

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
                Transaction::KeyValue { key, value } => {
                    self.write_value(transaction.signer(), key, value)?;
                }
            }
        }

        Ok(())
    }

    fn write_value(&self, peer_id: &PeerId, key: &str, value: &[u8]) -> Result<(), Error> {
        // Add the peer to the account db.
        self.accounts.insert(peer_id.as_bytes(), &[])?;

        // Add the value name the time_series tree.
        self.database
            .open_tree(peer_id.as_bytes())?
            .insert(key, &[])?;

        // Insert value with timestamp into the time_series tree.
        let time_series_name = [peer_id.as_bytes(), key.as_bytes()].join(&0);
        let time = timestamp_millis().to_be_bytes();
        self.database
            .open_tree(time_series_name)?
            .insert(time, value)?;

        Ok(())
    }

    /// Retreive the current `BlockNumber`.
    pub fn block_number(&self) -> Result<BlockNumber, Error> {
        match self.blocks.iter().keys().rev().next() {
            Some(key) => {
                let block_number = BlockNumber::from_be_bytes(key?).unwrap();
                Ok(block_number + 1)
            }
            None => Ok(BlockNumber::default()),
        }
    }

    /// Read a range of blocks from the store.
    pub fn read<R>(&self, range: R) -> impl DoubleEndedIterator<Item = Result<Block, Error>>
    where
        R: RangeBounds<BlockNumber>,
    {
        let start = range.start_bound();
        let end = range.end_bound();
        self.blocks
            .range((
                map_bound_from_block_number(start),
                map_bound_from_block_number(end),
            ))
            .values()
            .map(|result| {
                let value = result?;
                let block = postcard::from_bytes(&value)?;
                Ok(block)
            })
    }

    /// Remove the last block (at the end of the chain) and return it.
    pub fn pop_block(&self) -> Result<Option<Block>, Error> {
        if let Some((_, value)) = self.blocks.pop_max()? {
            let block: Block = postcard::from_bytes(&value)?;

            // update value tree
            for transaction in &block.body.transactions {
                match transaction.unverified_ref() {
                    Transaction::KeyValue { key, value: _ } => {
                        let peer_id = transaction.signer();
                        let time_series_name = [peer_id.as_bytes(), key.as_bytes()].join(&0);
                        self.database.open_tree(time_series_name)?.pop_max()?;
                    }
                }
            }

            Ok(Some(block))
        } else {
            Ok(None)
        }
    }
}

fn map_bound_from_block_number(bound: Bound<&BlockNumber>) -> Bound<impl AsRef<[u8]>> {
    map_bound(bound, |v| v.to_be_bytes())
}

fn map_bound<T, U>(bound: Bound<T>, f: impl FnOnce(T) -> U) -> Bound<U> {
    match bound {
        Bound::Included(v) => Bound::Included(f(v)),
        Bound::Excluded(v) => Bound::Excluded(f(v)),
        Bound::Unbounded => Bound::Unbounded,
    }
}

// FIXME: This is a copy from data_storage. Also the timestamp should come from the leader?
#[allow(clippy::cast_possible_truncation)]
fn timestamp_millis() -> i64 {
    match std::time::SystemTime::UNIX_EPOCH.elapsed() {
        Ok(duration) => duration.as_millis() as i64,
        Err(err) => -(err.duration().as_millis() as i64),
    }
}
