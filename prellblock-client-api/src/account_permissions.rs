//! Defines permissions for accounts.

use serde::{Deserialize, Serialize};

/// The right to read from specific accounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingRight {
    /// A black- or whitelist of accounts.
    accounts: ReadingPermission,

    /// The tree belonging to a account.
    namespace: ReadingPermission,
}

/// A `ReadingPermission` can be either a white- or a blacklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingPermission {
    /// A `Blacklist` of permissions.
    Blacklist(PermissionList),

    /// A `Whitelist` of permissions.
    Whitelist(PermissionList),
}

type PermissionList = Vec<Permission>;

/// A filter that can select a given scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permission {
    /// The scope of this filter.
    pub scope: String,
}

// pub enum Permission {
//     Exact(String),
//     Prefix(String),
//     RegEx(String), // ???
// }
