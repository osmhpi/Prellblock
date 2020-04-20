//! Module containing the `WorldState`-Component.

use chrono::prelude::*;
use pinxit::PeerId;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};

/// A `WorldState` keeps track of the current state of the blockchain.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct WorldState {
    pub(crate) accounts: HashMap<PeerId, Account>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Account {
    name: String,

    #[serde(default)]
    is_rpu: bool,

    #[serde(default)]
    expire_at: Option<DateTime<Utc>>,

    #[serde(default)]
    pub(crate) writing_rights: bool,

    #[serde(default)]
    reading_rights: Vec<ReadingRight>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ReadingRight {
    keyspace: ReadingPermission,
    namespace: ReadingPermission,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReadingPermission {
    Blacklist(PermissionList),
    Whitelist(PermissionList),
}
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct PermissionList {
    permissions: Vec<Permission>,
}
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Permission {
    scope: String,
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
        Self { accounts }
    }
}
