//! Module containing the `WorldState`-Component.

#![allow(clippy::module_name_repetitions)]

mod account;

pub(crate) use account::Account;

use im::HashMap;
use pinxit::PeerId;
use serde::{Deserialize, Serialize};
use std::{
    fmt, fs,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Struct holding a `WorldState` mutex.
#[derive(Debug, Clone)]
#[must_use]
pub struct WorldStateService {
    world_state: Arc<Mutex<WorldState>>,
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
    pub fn with_world_state(world_state: WorldState) -> Self {
        Self {
            world_state: Arc::new(world_state.into()),
            writer: Arc::new(Semaphore::new(1)),
        }
    }

    /// Create a new `WorldStateService`.
    pub fn new() -> Self {
        let world_state = WorldState::default();
        Self::with_world_state(world_state)
    }

    /// Return a copy of the entire `WorldState`.
    #[must_use]
    pub fn get(&self) -> WorldState {
        self.world_state.lock().unwrap().clone()
    }

    /// Return a copy of the entire `WorldState`.
    pub async fn get_writable(&self) -> WritableWorldState {
        let permit = self.writer.clone().acquire_owned().await;
        WritableWorldState {
            shared_world_state: self.world_state.clone(),
            world_state: self.world_state.lock().unwrap().clone(),
            permit,
        }
    }
}

/// A writable copy of the `WorldState`. Can be edited and later `save`d to the global `WorldState`
#[derive(Debug)]
#[must_use]
pub struct WritableWorldState {
    shared_world_state: Arc<Mutex<WorldState>>,
    world_state: WorldState,
    #[allow(dead_code)]
    permit: OwnedSemaphorePermit,
}

impl WritableWorldState {
    /// Save the cahnged `WorldState`.
    pub fn save(self) {
        *self.shared_world_state.lock().unwrap() = self.world_state;
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
    pub accounts: HashMap<PeerId, Account>,
    /// Field storing the Transactiondata.
    pub data: HashMap<PeerId, HashMap<String, Vec<u8>>>,
}

impl WorldState {
    /// Function used for developement purposes, loads static accounts from a config file.
    #[must_use]
    pub fn with_fake_data() -> Self {
        let yaml_file = fs::read_to_string("./config/accounts.yaml").unwrap();
        let accounts_strings: HashMap<String, Account> = serde_yaml::from_str(&yaml_file).unwrap();

        let accounts = accounts_strings
            .into_iter()
            .map(|(key, account)| (key.parse().expect("peer_id in accounts.yaml"), account))
            .collect();
        Self {
            accounts,
            data: HashMap::new(),
        }
    }
}

impl fmt::Display for WorldState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
