//! The datastorage is a temporary storage for incoming transactions persisted on disk.
use chrono::{DateTime, Utc};
use pinxit::PeerId;
use sled::{Config, Db, IVec, Tree};

const ROOT_TREE_NAME: &[u8; 4] = b"root";

// TODO: Remove this.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A `DataStorage` provides persistent storage on disk.
///
/// Data is written to disk every 400ms.
pub struct DataStorage {
    database: Db,
    root: Tree,
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
        let tree_root = database.open_tree(ROOT_TREE_NAME)?;

        Ok(Self {
            root: tree_root,
            database,
        })
    }

    /// Write a value to the store.
    ///
    /// The data will be associated with the peer via its id.
    pub fn write<K>(&self, peer: &PeerId, key: K, value: &serde_json::Value) -> Result<(), BoxError>
    where
        K: AsRef<[u8]>,
    {
        // find id for peer tree
        let peer_tree = self.tree_for_name(&self.root, peer.hex())?;

        // find id for key tree
        let key_tree = self.tree_for_name(&peer_tree, key)?;

        // insert value with timestamp
        let now: DateTime<Utc> = Utc::now();
        let time = now.timestamp_millis().to_be_bytes();
        let value = serde_json::to_vec(&value)?;
        key_tree.insert(&time, value)?;

        // Demo that it's working.
        // log::debug!("Data storage now has {} entries.", key_tree.len());
        // for value in key_tree.iter() {
        //     if let Ok((key, value)) = value {
        //         log::trace!("Found {:x?} => {:x?}", key, value);
        //     }
        // }

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
}
