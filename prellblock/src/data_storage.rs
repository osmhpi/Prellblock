//! The `DataStorage` is a temporary storage for incoming transactions persisted on disk.

use hexutil::ToHex;
use pinxit::PeerId;
use prellblock_client_api::transaction::UpdateAccount;
use sled::{Config, Db, IVec, Tree};

use crate::BoxError;

const KEY_VALUE_ROOT_TREE_NAME: &[u8] = b"root";
const ACCOUNTS_TREE_NAME: &[u8] = b"accounts";

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
            .snapshot_after_ops(100)
            .use_compression(false) // TODO: set this to `true`.
            .compression_factor(20);

        let database = config.open()?;
        let key_value_root = database.open_tree(KEY_VALUE_ROOT_TREE_NAME)?;
        let accounts = database.open_tree(ACCOUNTS_TREE_NAME)?;

        Ok(Self {
            database,
            key_value_root,
            accounts,
        })
    }

    /// Write a value to the data storage.
    ///
    /// The data will be associated with the peer via its `PeerId`.
    pub fn write_key_value<K>(&self, peer: &PeerId, key: K, value: &[u8]) -> Result<(), BoxError>
    where
        K: AsRef<[u8]>,
    {
        // find id for peer tree
        let peer_tree = self.tree_for_name(&self.key_value_root, peer.to_hex())?;

        // find id for key tree
        let key_tree = self.tree_for_name(&peer_tree, key)?;

        // insert value with timestamp
        let time = timestamp_millis().to_be_bytes();
        let value = postcard::to_stdvec(&value)?;
        key_tree.insert(&time, value)?;

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

    /// Write an `UpdateAccount` transaction to the data storage.
    ///
    /// The data will be associated with the sender peer via its `PeerId`.
    pub fn write_account_update(
        &self,
        peer: &PeerId,
        transaction: &UpdateAccount,
    ) -> Result<(), BoxError> {
        // find tree for sender account
        let peer_tree = self.tree_for_name(&self.accounts, peer.to_hex())?;

        // insert update transaction with timestamp
        let time = timestamp_millis().to_be_bytes();
        let transaction = postcard::to_stdvec(transaction)?;
        peer_tree.insert(time, transaction)?;

        Ok(())
    }
}

// We do not expect a system time that far off:
#[allow(clippy::cast_possible_truncation)]
fn timestamp_millis() -> i64 {
    match std::time::SystemTime::UNIX_EPOCH.elapsed() {
        Ok(duration) => duration.as_millis() as i64,
        Err(err) => -(err.duration().as_millis() as i64),
    }
}
