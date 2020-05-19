//! Defines permissions for accounts.

use chrono::prelude::*;
use serde::{Deserialize, Serialize};

/// Permission fields for a account.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    /// Whether the account shall be an admin.
    pub is_admin: Option<bool>,
    /// Whether the account shall be a RPU.
    pub is_rpu: Option<bool>,
    /// Expiry of the account.
    pub expire_at: Option<Expiry>,
    /// Whether the account shall have permissions to write into its namespace.
    pub has_writing_rights: Option<bool>,
    /// Permissions for reading the namespaces of other accounts.
    pub reading_rights: Option<Vec<ReadingRight>>,
}

/// An accounts permission can either be `never` expiring or expiring at a certain date (`AtDate`).
///
/// # Example
/// ```
/// use chrono::{Duration, Utc};
/// use prellblock_client_api::account_permissions::Expiry;
///
/// let never_expired = Expiry::Never;
/// assert_eq!(never_expired.is_expired(), false);
///
/// let not_expired = Expiry::AtDate(Utc::now() + Duration::days(1));
/// assert_eq!(not_expired.is_expired(), false);
///
/// let already_expired = Expiry::AtDate(Utc::now() - Duration::days(1));
/// assert_eq!(already_expired.is_expired(), true);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expiry {
    /// The permission never expires.
    Never,
    /// The permission expires at the given date.
    AtDate(DateTime<Utc>),
}

impl Expiry {
    /// Check whether the expiry date has passed (if set).
    #[must_use]
    pub fn is_expired(&self) -> bool {
        match self {
            Self::Never => false,
            Self::AtDate(expiry) => Utc::now() > *expiry,
        }
    }
}

impl Default for Expiry {
    fn default() -> Self {
        Self::Never
    }
}

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
