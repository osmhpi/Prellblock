//! Module containing the `WorldState`-Component.

#![allow(clippy::module_name_repetitions)]

pub use prellblock_client_api::account::Account;

use crate::{
    block_storage::BlockStorage,
    consensus::{Block, BlockHash, BlockNumber},
    BoxError,
};
use im::{HashMap, Vector};
use pinxit::{PeerId, Signed};
use prellblock_client_api::Transaction;
use serde::{Deserialize, Serialize};
use std::{
    fmt, fs,
    net::SocketAddr,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Struct holding a `Worldstate` and it's previous `Worldstate`, if any.
#[derive(Debug, Default)]
pub struct WorldStateReferences {
    current: WorldState,
    prev: Option<WorldState>,
}

/// Struct holding a `WorldState` mutex.
#[derive(Debug, Clone)]
#[must_use]
pub struct WorldStateService {
    world_state_references: Arc<Mutex<WorldStateReferences>>,
    writer: Arc<Semaphore>,
}

impl fmt::Display for WorldStateService {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}

impl Default for WorldStateService {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldStateService {
    /// Create a new `WorldStateService` initalized with a given `world_state`.
    fn with_world_state_references(world_state_references: WorldStateReferences) -> Self {
        Self {
            world_state_references: Arc::new(world_state_references.into()),
            writer: Arc::new(Semaphore::new(1)),
        }
    }

    /// Create a new `WorldStateService` initalized with the blocks from a `block_storage`.
    pub fn from_block_storage(
        block_storage: &BlockStorage,
        peer_accounts: impl Iterator<Item = (PeerId, Account)>,
        peers: Vector<(PeerId, SocketAddr)>,
    ) -> Result<Self, BoxError> {
        let mut world_state_references = WorldStateReferences::default();

        // TODO: Remove this. Currently for development purposes.
        {
            let world_state = &mut world_state_references.current;
            world_state.load_fake_accounts();
            world_state.accounts.extend(peer_accounts);
            world_state.peers = peers;
        }

        let mut blocks = block_storage.read(..);
        let last_block = blocks.next_back();
        for block in blocks {
            world_state_references.current.apply_block(block?)?;
        }

        if let Some(last_block) = last_block {
            world_state_references.prev = Some(world_state_references.current.clone());
            world_state_references.current.apply_block(last_block?)?;
        }

        log::debug!("Current WorldState: {:#}", world_state_references.current);

        Ok(Self::with_world_state_references(world_state_references))
    }

    /// Create a new `WorldStateService`.
    pub fn new() -> Self {
        let world_state_references = WorldStateReferences::default();
        Self::with_world_state_references(world_state_references)
    }

    /// Return a copy of the entire `WorldState`.
    #[must_use]
    pub fn get(&self) -> WorldState {
        self.world_state_references.lock().unwrap().current.clone()
    }
    /// Rollback the `WorldState` to the previous state.
    #[allow(clippy::must_use_candidate)]
    pub fn rollback(&self) -> Option<WorldState> {
        let mut world_state_references = self.world_state_references.lock().unwrap();
        let previous = world_state_references.prev.take()?;
        let old_current = std::mem::replace(&mut world_state_references.current, previous);
        Some(old_current)
    }

    /// Return a copy of the entire `WorldState`.
    pub async fn get_writable(&self) -> WritableWorldState {
        let permit = self.writer.clone().acquire_owned().await;
        WritableWorldState {
            shared_world_state: self.world_state_references.clone(),
            world_state: self.get(),
            permit,
        }
    }
}

/// A writable copy of the `WorldState`. Can be edited and later `save`d to the global `WorldState`
#[derive(Debug)]
#[must_use]
pub struct WritableWorldState {
    shared_world_state: Arc<Mutex<WorldStateReferences>>,
    world_state: WorldState,
    #[allow(dead_code)]
    permit: OwnedSemaphorePermit,
}

impl WritableWorldState {
    /// Save the cahnged `WorldState`.
    pub fn save(self) {
        log::trace!("Changed WorldState: {:#}", self.world_state);
        let mut world_state_references = self.shared_world_state.lock().unwrap();
        world_state_references.prev = Some(world_state_references.current.clone());
        world_state_references.current = self.world_state;
    }
}

impl fmt::Display for WritableWorldState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.world_state)
    }
}

impl Deref for WritableWorldState {
    type Target = WorldState;
    fn deref(&self) -> &Self::Target {
        &self.world_state
    }
}

impl DerefMut for WritableWorldState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.world_state
    }
}

/// A `WorldState` keeps track of the current state of the blockchain.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WorldState {
    /// Field storing the `Account` `Permissions`.
    pub accounts: HashMap<PeerId, Arc<Account>>,
    /// Field storing the `Peer`s.
    pub peers: Vector<(PeerId, SocketAddr)>,
    /// The number of `Block`s applied to the `WorldState`.
    pub block_number: BlockNumber,
    /// Hash of the last `Block` in the `BlockStorage`.
    pub last_block_hash: BlockHash,
}

impl WorldState {
    /// Function used for developement purposes, loads static accounts from a config file.
    fn load_fake_accounts(&mut self) {
        let yaml_file = fs::read_to_string("./config/accounts.yaml").unwrap();
        let accounts_strings: HashMap<String, Account> = serde_yaml::from_str(&yaml_file).unwrap();

        self.accounts = accounts_strings
            .into_iter()
            .map(|(key, account)| {
                (
                    key.parse().expect("peer_id in accounts.yaml"),
                    account.into(),
                )
            })
            .collect();
    }

    /// Apply a block to the current world state.
    pub fn apply_block(&mut self, block: Block) -> Result<(), BoxError> {
        if block.body.prev_block_hash != self.last_block_hash {
            return Err("Last block hash is not equal to hash of last block.".into());
        }
        // TODO: validate block (peers, signatures, etc)
        self.last_block_hash = block.body.hash();
        self.block_number = block.body.height + 1;
        for transaction in block.body.transactions {
            self.apply_transaction(transaction);
        }
        Ok(())
    }

    /// Apply a transaction to the current world state.
    pub fn apply_transaction(&mut self, transaction: Signed<Transaction>) {
        match transaction.unverified() {
            Transaction::KeyValue(_) => {}
            Transaction::UpdateAccount(params) => {
                if let Some(account) = self.accounts.get_mut(&params.id).map(Arc::make_mut) {
                    let permissions = params.permissions;
                    if let Some(account_type) = permissions.account_type {
                        account.account_type = account_type;
                    }
                    if let Some(expire_at) = permissions.expire_at {
                        account.expire_at = expire_at;
                    }
                    if let Some(writing_rights) = permissions.has_writing_rights {
                        account.writing_rights = writing_rights;
                    }
                    if let Some(reading_rights) = permissions.reading_rights {
                        account.reading_rights = reading_rights;
                    }
                } else {
                    unreachable!("Account {} does not exist.", params.id);
                }
            }
        }
    }
}

impl fmt::Display for WorldState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
