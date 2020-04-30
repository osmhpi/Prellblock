//! The `BlockStorage` is a permantent storage for validated Blocks persisted on disk.

use crate::{
    consensus::{Block, BlockHash, SequenceNumber},
    BoxError,
};

use sled::{Config, Tree};
use std::ops::{Bound, RangeBounds};
// use sled::Db;
const BLOCKS_TREE_NAME: &[u8] = b"blocks";

/// A `BlockStorage` provides persistent storage on disk.
///
/// Data is written to disk every 400ms.
pub struct BlockStorage {
    // database: Db,
    blocks: Tree,
}

impl BlockStorage {
    /// Create a new `Store` at path.
    pub fn new(path: &str) -> Result<Self, BoxError> {
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
    /// The data will be accessible by the sequence number?.
    pub fn write_block(&self, block: &Block) -> Result<(), BoxError> {
        let (last_block_hash, last_block_height) =
            if let Some(last_block) = self.read(..).next_back() {
                let last_block = last_block?;
                (last_block.hash(), last_block.body.height)
            } else {
                (BlockHash::default(), 0)
            };

        if last_block_hash != block.body.prev_block_hash {
            return Err("Block hash does not match the previous block hash.".into());
        }

        if last_block_height + 1 != block.body.height {
            return Err("Block height does not fit the previous block height.".into());
        }

        let value = postcard::to_stdvec(&block)?;
        self.blocks
            .insert(block.sequence_number().to_be_bytes(), value)?;
        Ok(())
    }

    /// Read a range of blocks from the store.
    pub fn read<R>(&self, range: R) -> impl DoubleEndedIterator<Item = Result<Block, BoxError>>
    where
        R: RangeBounds<SequenceNumber>,
    {
        let start = range.start_bound();
        let end = range.end_bound();
        self.blocks
            .range((
                map_bound_from_sequence_number(start),
                map_bound_from_sequence_number(end),
            ))
            .values()
            .map(|result| {
                let value = result?;
                let block = postcard::from_bytes(&value)?;
                Ok(block)
            })
    }
}

fn map_bound_from_sequence_number(bound: Bound<&SequenceNumber>) -> Bound<[u8; 8]> {
    map_bound(bound, |v| v.to_be_bytes())
}

fn map_bound<T, U>(bound: Bound<T>, f: impl FnOnce(T) -> U) -> Bound<U> {
    match bound {
        Bound::Included(v) => Bound::Included(f(v)),
        Bound::Excluded(v) => Bound::Excluded(f(v)),
        Bound::Unbounded => Bound::Unbounded,
    }
}
