//! Can be used by any consensus algorithm to apply blocks.

use super::{Block, Error};
use crate::{block_storage::BlockStorage, world_state::WorldStateService};
use http::StatusCode;
use prellblock_client_api::Transaction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
        self.apply_to_worldstate(block.clone()).await;
        // export Block using HTTP POST request
        // #[cfg(thingsboard)]
        let _ = self.post_block(block).await;
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

    /// Sends a `Block` via a HTTP POST request to an address specified in the config.
    pub async fn post_block(&self, block: Block) -> Result<(), Error> {
        // serialize block
        // let values: Vec<_> = block
        //     .body
        //     .transactions
        //     .iter()
        //     .map(|signed| (signed.signer(), signed.unverified_ref()))
        //     .collect();
        let mut values = Vec::new();
        for transaction in &block.body.transactions {
            match transaction.unverified_ref() {
                Transaction::KeyValue(params) => {
                    let key = format!("{}-{}", transaction.signer(), params.key);
                    // let mut value = [0; 4];
                    // value.copy_from_slice(&params.value[0..4]);
                    let value: f64 = postcard::from_bytes(&params.value).unwrap();
                    // let value = u32::from_le_bytes(value);
                    let mut key_value = HashMap::new();
                    key_value.insert(key, value);
                    values.push(key_value);
                }
                Transaction::UpdateAccount(params) => {}
            }
        }
        let values = serde_json::to_string(&values)?;

        // setup request
        let client = reqwest::Client::new();
        let host = "localhost";
        let port = "8080";
        let access_token = "dtcisBXItTT4cEkg5EpM";
        let url = format!("http://{}:{}/api/v1/{}/telemetry", host, port, access_token);
        let values = values.replace("\"", "");
        log::trace!("POST: {:?}", values);
        let body = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(values);

        //send request
        let res = body.send().await?;
        match res.status() {
            StatusCode::OK => log::trace!("Send POST successfully."),
            StatusCode::BAD_REQUEST => log::warn!("BAD_REQUEST response from {}.", host),
            _ => {
                log::trace!("Statuscode: {:?}", res.status());
            }
        }
        Ok(())
    }
}
