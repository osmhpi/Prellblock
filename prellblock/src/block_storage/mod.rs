//! The `BlockStorage` is a permantent storage for validated Blocks persisted on disk.

use crate::{consensus::Block, BoxError};

use sled::{Config, Tree};
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
        let value = serde_json::to_vec(&block)?;

        self.blocks
            .insert(block.sequence_number().to_be_bytes(), value)?;

        Ok(())
    }
}
