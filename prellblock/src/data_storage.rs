//! The `DataStorage` is a temporary storage for incoming transactions persisted on disk.

use hexutil::ToHex;
use pinxit::PeerId;
use prellblock_client_api::AccountTransaction;
use serde::Serialize;
use sled::{Config, Db, IVec, Tree};
use std::time::SystemTime;

use crate::{if_monitoring, time, BoxError};

const KEY_VALUE_ROOT_TREE_NAME: &[u8] = b"root";
const ACCOUNTS_TREE_NAME: &[u8] = b"accounts";

if_monitoring! {
    use lazy_static::lazy_static;
use prometheus::{register_histogram, register_int_gauge, Histogram, IntGauge};
lazy_static! {
    /// Measure the number of transactions in the DataStorage.
    static ref TRANSACTIONS_IN_DATA_STORAGE: IntGauge = register_int_gauge!(
        "data_storage_num_txs",
        "The aggregated number of transactions in the DataStorage."
    )
    .unwrap();

    /// Measure the time a transaction takes from being created on the client until it reaches the DataStorage.
    static ref DATASTORAGE_ARRIVAL_TIME: Histogram = register_histogram!(
        "datastorage_arrival_time",
        "The time a transaction takes from being created by the client until it reaches the DataStorage.",
        vec![0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.7,0.75,0.8,0.85,0.9,0.95,1.0,1.05,1.1,]
    ).unwrap();

    /// Measure the size of the DataStorage on the disk.
    static ref DATASTORAGE_SIZE: IntGauge = register_int_gauge!(
        "datastorage_size",
        "Size of the DataStorage on the disk."
    )
    .unwrap();
}
}

/// A `DataStorage` provides persistent storage on disk.
///
/// Data is written to disk every 400ms.
pub struct DataStorage {
    database: Db,
    key_value_root: Tree,
    accounts: Tree,
}

impl DataStorage {
    /// Create a new `Store` at path.
    pub fn new(path: &str) -> Result<Self, BoxError> {
        let config = Config::default()
            .path(path)
            .cache_capacity(8_000_000)
            .flush_every_ms(Some(400))
            .snapshot_after_ops(1000000)
            .use_compression(false) // TODO: set this to `true`.
            .compression_factor(20);

        let database = config.open()?;
        let key_value_root = database.open_tree(KEY_VALUE_ROOT_TREE_NAME)?;
        let accounts = database.open_tree(ACCOUNTS_TREE_NAME)?;

        let data_storage = Self {
            database,
            key_value_root,
            accounts,
        };
        if_monitoring!({
            #[allow(clippy::cast_possible_wrap)]
            match data_storage.database.size_on_disk() {
                Ok(size) => DATASTORAGE_SIZE.set(size as i64),
                Err(err) => log::warn!("Error calculating size of datastorage on disk: {}", err),
            }
            TRANSACTIONS_IN_DATA_STORAGE.set(data_storage.count_transactions())
        });

        Ok(data_storage)
    }

    /// Count all transactions in the store.
    fn count_transactions(&self) -> i64 {
        let mut size = 0;

        // Count all `Account` transactions.
        for peer_tree_tuple in self.accounts.iter() {
            if let Ok((_, peer_account_tree_id)) = peer_tree_tuple {
                size += self.database.open_tree(peer_account_tree_id).unwrap().len();
            }
        }

        // Count all key-value transactions.
        for peer_tree_tuple in self.key_value_root.iter() {
            if let Ok((_, peer_key_tree_id)) = peer_tree_tuple {
                let key_tree = self.database.open_tree(peer_key_tree_id).unwrap();
                for key_tuple in key_tree.iter() {
                    if let Ok((_, series_id)) = key_tuple {
                        size += self.database.open_tree(series_id).unwrap().len();
                    }
                }
            }
        }

        size as i64
    }

    /// Write a value to the data storage.
    ///
    /// The data will be associated with the peer via its `PeerId`.
    pub fn write_key_value<K>(
        &self,
        peer: &PeerId,
        key: K,
        value: &[u8],
        timestamp: SystemTime,
    ) -> Result<(), BoxError>
    where
        K: AsRef<[u8]>,
    {
        // find id for peer tree
        let peer_tree = self.tree_for_name(&self.key_value_root, peer.to_hex())?;

        // find id for key tree
        let key_tree = self.tree_for_name(&peer_tree, key)?;

        // insert value with timestamp
        let time = SystemTime::now();
        let value = postcard::to_stdvec(&(value, timestamp))?;

        if_monitoring!({
            match time.duration_since(timestamp) {
                Ok(duration) => DATASTORAGE_ARRIVAL_TIME.observe(duration.as_secs_f64()),
                Err(err) => log::warn!("Error calculating duration in datastorage: {}", err),
            }
            #[allow(clippy::cast_possible_wrap)]
            match self.database.size_on_disk() {
                Ok(size) => DATASTORAGE_SIZE.set(size as i64),
                Err(err) => log::warn!("Error calculating size of datastorage on disk: {}", err),
            }
            TRANSACTIONS_IN_DATA_STORAGE.inc();
        });

        key_tree.insert(time::system_time_to_bytes(time), value)?;

        Ok(())
    }

    /// Generate a new id for naming trees.
    fn new_tree_id(&self) -> Result<IVec, BoxError> {
        let id = self.database.generate_id()?;
        Ok(id.to_be_bytes().to_vec().into())
    }

    /// Find a tree by its Id in a given tree.
    ///
    /// This function will search for the tree's name in `in_tree`.
    /// If no tree is found, one will be opened and inserted into `in_tree` with a freshly generated id.
    fn tree_for_name<N>(&self, in_tree: &Tree, name: N) -> Result<Tree, BoxError>
    where
        N: AsRef<[u8]>,
    {
        let tree_id = if let Some(tree_id) = in_tree.get(&name)? {
            tree_id
        } else {
            let new_tree_id = self.new_tree_id()?;
            in_tree.insert(&name, &new_tree_id)?;
            new_tree_id
        };
        Ok(self.database.open_tree(&tree_id)?)
    }

    /// Write an `UpdateAccount`, `DeleteAccount` or `CreateAccount` transaction to the data storage.
    ///
    /// The data will be associated with the sender peer via its `PeerId`.
    pub fn write_account_transaction<T>(
        &self,
        peer: &PeerId,
        transaction: &T,
    ) -> Result<(), BoxError>
    where
        T: AccountTransaction + Serialize,
    {
        // find tree for sender account
        let peer_tree = self.tree_for_name(&self.accounts, peer.to_hex())?;

        let time = SystemTime::now();
        if_monitoring!({
            match time.duration_since(transaction.timestamp()) {
                Ok(duration) => DATASTORAGE_ARRIVAL_TIME.observe(duration.as_secs_f64()),
                Err(err) => log::warn!(
                    "Error calculating duration in datastorage for monitoring: {}",
                    err
                ),
            }
            #[allow(clippy::cast_possible_wrap)]
            match self.database.size_on_disk() {
                Ok(size) => DATASTORAGE_SIZE.set(size as i64),
                Err(err) => log::warn!("Error calculating size of datastorage on disk: {}", err),
            }
            TRANSACTIONS_IN_DATA_STORAGE.inc();
        });

        // insert update transaction with timestamp
        let transaction = postcard::to_stdvec(transaction)?;
        peer_tree.insert(time::system_time_to_bytes(time), transaction)?;

        Ok(())
    }
}
