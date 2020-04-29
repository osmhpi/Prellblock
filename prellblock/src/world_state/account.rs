use chrono::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    name: String,

    #[serde(default)]
    is_rpu: bool,

    #[serde(default)]
    expire_at: Option<DateTime<Utc>>,

    #[serde(default)]
    pub writing_rights: bool,

    #[serde(default)]
    reading_rights: Vec<ReadingRight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingRight {
    /// a black- or whitelist of accounts
    accounts: ReadingPermission,
    /// the tree belonging to a account
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
