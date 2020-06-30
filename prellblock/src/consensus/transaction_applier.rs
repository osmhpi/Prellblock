//! Can be used by any consensus algorithm to apply blocks.

use super::Block;
#[cfg(feature = "subscriptions")]
use crate::subscriptions::SubscriptionManager;
use crate::{block_storage::BlockStorage, world_state::WorldStateService};
#[cfg(feature = "subscriptions")]
use prellblock_client_api::Transaction;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct KeyValue {
    key: String,
    value: String,
}

/// Helps to apply transactions onto the `BlockStorage` and `WorldState`.
#[derive(Debug)]
pub struct TransactionApplier {
    block_storage: BlockStorage,
    world_state: WorldStateService,
    #[cfg(feature = "subscriptions")]
    subscription_manager: SubscriptionManager,
}

impl TransactionApplier {
    /// Create a new `TransactionApplier` instance.
    #[must_use]
    pub const fn new(
        block_storage: BlockStorage,
        world_state: WorldStateService,
        #[cfg(feature = "subscriptions")] subscription_manager: SubscriptionManager,
    ) -> Self {
        Self {
            block_storage,
            world_state,
            #[cfg(feature = "subscriptions")]
            subscription_manager,
        }
    }

    /// Applies a given to both the `world_state` and the `block_storage`.
    pub async fn apply_block(&self, block: Block) {
        // Write Block to BlockStorage
        self.apply_to_block_storage(&block);

        // Write Block to WorldState
        self.apply_to_worldstate(block.clone()).await;
        // export data using HTTP POST request
        #[cfg(feature = "subscriptions")]
        self.notify_block_update(block).await;
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

    #[cfg(feature = "subscriptions")]
    async fn notify_block_update(&self, block: Block) {
        let mut values = Vec::new();
        for transaction in &block.body.transactions {
            match transaction.unverified_ref() {
                Transaction::KeyValue(params) => {
                    values.push((transaction.signer().clone(), params.key.clone()));
                }
                Transaction::UpdateAccount(_) => {}
            }
        }
        // FIXME: This could break with sled's `open_tree`, does it?
        self.subscription_manager
            .notify_block_update(values)
            .await
            .unwrap();
    }
}
