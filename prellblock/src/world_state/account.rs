use chrono::prelude::*;
use serde::{Deserialize, Serialize};

/// `Account` stores data needed for permission checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// The `Account`'s name.
    pub name: String,

    /// Whether the `Account`'s is an rpu. (Default `false`).
    #[serde(default)]
    pub is_rpu: bool,

    /// The `Account`'s expiring date. (Default `None`).
    #[serde(default)]
    pub expire_at: Option<DateTime<Utc>>,

    /// The `Account`'s writing rights. (Default `false`).
    #[serde(default)]
    pub writing_rights: bool,

    /// The `Account`'s reading rights. (Default `Vec::new()`).
    #[serde(default)]
    pub reading_rights: Vec<ReadingRight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingRight {
    /// A black- or whitelist of accounts.
    accounts: ReadingPermission,
    /// The tree belonging to a account.
    namespace: ReadingPermission,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingPermission {
    Blacklist(PermissionList),
    Whitelist(PermissionList),
}

type PermissionList = Vec<Permission>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    scope: String,
}
