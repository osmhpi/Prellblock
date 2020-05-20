//! The `BlockStorage` is a permantent storage for validated Blocks persisted on disk.

mod error;

pub use error::Error;

use crate::consensus::{Block, BlockHash, BlockNumber};

use sled::{Config, Tree};
use std::ops::{Bound, RangeBounds};
// use sled::Db;
const BLOCKS_TREE_NAME: &[u8] = b"blocks";

/// A `BlockStorage` provides persistent storage on disk.
///
/// Data is written to disk every 400ms.
#[derive(Debug, Clone)]
pub struct BlockStorage {
    // database: Db,
    blocks: Tree,
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

        Ok(Self {
            // database,
            blocks,
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
        Ok(())
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
            let block = postcard::from_bytes(&value)?;
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
