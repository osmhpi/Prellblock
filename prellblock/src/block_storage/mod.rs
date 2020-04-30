//! The `BlockStorage` is a permantent storage for validated Blocks persisted on disk.

use crate::{consensus::Block, BoxError};

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
        let value = postcard::to_stdvec(&block)?;
        self.blocks
            .insert(block.sequence_number().to_be_bytes(), value)?;
        Ok(())
    }

    /// Read a range of blocks from the store.
    pub fn read<R>(&self, range: R) -> impl Iterator<Item = Result<Block, BoxError>>
    where
        R: RangeBounds<u64>,
    {
        let start = range.start_bound();
        let end = range.end_bound();
        self.blocks
            .range((map_bound_from_u64(start), map_bound_from_u64(end)))
            .values()
            .map(|result| {
                let value = result?;
                let block = postcard::from_bytes(&value)?;
                Ok(block)
            })
    }
}

fn map_bound_from_u64(bound: Bound<&u64>) -> Bound<[u8; 8]> {
    map_bound(bound, |v| v.to_be_bytes())
}

fn map_bound<T, U>(bound: Bound<T>, f: impl FnOnce(T) -> U) -> Bound<U> {
    match bound {
        Bound::Included(v) => Bound::Included(f(v)),
        Bound::Excluded(v) => Bound::Excluded(f(v)),
        Bound::Unbounded => Bound::Unbounded,
    }
}
