//! Can be used by any consensus algorithm to apply blocks.

use super::Block;
use crate::{block_storage::BlockStorage, world_state::WorldStateService};

/// Helps to apply transactions onto the `BlockStorage` and `WorldState`.
#[derive(Debug)]
pub struct TransactionApplier {
    block_storage: BlockStorage,
    world_state: WorldStateService,
}

impl TransactionApplier {
    /// Create a new `TransactionApplier` instance.
    #[must_use]
    pub const fn new(block_storage: BlockStorage, world_state: WorldStateService) -> Self {
        Self {
            block_storage,
            world_state,
        }
    }

    /// Applies a given to both the `world_state` and the `block_storage`.
    pub async fn apply_block(&self, block: Block) {
        // Write Block to BlockStorage
        self.apply_to_block_storage(&block);
        // Write Block to WorldState
        self.apply_to_worldstate(block).await;
    }

    /// Applies a given block to the `BlockStorage`.
    pub fn apply_to_block_storage(&self, block: &Block) {
        // Write Block to BlockStorage
        self.block_storage.write_block(block).unwrap();
    }

    /// Applies a given block to the `WorldState`.
    pub async fn apply_to_worldstate(&self, block: Block) {
        // Write Block to WorldState
        let mut world_state = self.world_state.get_writable().await;
        world_state.apply_block(block).unwrap();
        world_state.save();
    }
}
