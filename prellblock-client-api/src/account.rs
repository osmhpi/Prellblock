//! This module contains basic structures for `Account`s.

use chrono::prelude::*;
use pinxit::PeerId;
use serde::{Deserialize, Serialize};

/// `Account` stores data needed for permission checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// The `Account`'s name.
    pub name: String,

    /// The account type. (Default `AccountType::Normal`)
    #[serde(default, rename = "type")]
    pub account_type: AccountType,

    /// The `Account`'s expiring date. (Default `Expiry::Never`).
    #[serde(default)]
    pub expire_at: Expiry,

    /// The `Account`'s writing rights. (Default `false`).
    /// When set to `true`, the account is allowed to write into its own namespace.
    #[serde(default)]
    pub writing_rights: bool,

    /// The `Account`'s reading rights. (Default `Vec::new()`).
    #[serde(default)]
    pub reading_rights: Vec<ReadingPermission>,
}

impl Account {
    /// Create a new `Account` with a given name and default values.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            account_type: AccountType::default(),
            expire_at: Expiry::default(),
            writing_rights: false,
            reading_rights: Vec::new(),
        }
    }

    /// Apply `permissions` onto the account.
    pub fn apply_permissions(&mut self, permissions: Permissions) {
        if let Some(account_type) = permissions.account_type {
            self.account_type = account_type;
        }
        if let Some(expire_at) = permissions.expire_at {
            self.expire_at = expire_at;
        }
        if let Some(writing_rights) = permissions.has_writing_rights {
            self.writing_rights = writing_rights;
        }
        if let Some(reading_rights) = permissions.reading_rights {
            self.reading_rights = reading_rights;
        }
    }
}

/// Permission fields for a account.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    /// The account type.
    #[serde(rename = "type")]
    pub account_type: Option<AccountType>,
    /// Expiry of the account.
    pub expire_at: Option<Expiry>,
    /// Whether the account shall have permissions to write into its namespace.
    pub has_writing_rights: Option<bool>,
    /// Permissions for reading the namespaces of other accounts.
    pub reading_rights: Option<Vec<ReadingPermission>>,
}

/// The type of an account.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[allow(clippy::module_name_repetitions)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    /// A normal account with no special privileges.
    Normal,
    /// An acccount that can read whole blocks and therefore read all values.
    BlockReader,
    /// An RPU that can participate in the consensus.
    #[serde(rename = "rpu")]
    RPU {
        /// The address on which the `Turi` listens for incoming client requests.
        turi_address: String,
        /// The address on which the `PeerInbox` listens for incoming RPU-RPU communication.
        peer_address: String,
    },
    /// An admin that can manage and edit all other accounts.
    Admin,
}

impl AccountType {
    /// Whether the account type is AccountType::RPU
    pub fn is_rpu(&self) -> bool {
        match self {
            Self::RPU { .. } => true,
            _ => false,
        }
    }
}

impl Default for AccountType {
    fn default() -> Self {
        Self::Normal
    }
}

/// An accounts permission can either be `never` expiring or expiring at a certain date (`AtDate`).
///
/// # Example
/// ```
/// use chrono::{Duration, Utc};
/// use prellblock_client_api::account::Expiry;
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

/// A `ReadingPermission` can be either a white- or a blacklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingPermission {
    /// A `Blacklist` of permissions.
    Blacklist(ReadingRight),

    /// A `Whitelist` of permissions.
    Whitelist(ReadingRight),
}

/// The right to read from specific accounts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingRight {
    /// A black- or whitelist of accounts.
    pub accounts: Vec<PeerId>,

    /// The tree belonging to a account.
    pub namespace: Vec<Permission>,
}

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
