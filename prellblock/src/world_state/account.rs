use chrono::prelude::*;
use prellblock_client_api::account_permissions::ReadingRight;
use serde::{Deserialize, Serialize};

/// `Account` stores data needed for permission checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// The `Account`'s name.
    pub name: String,

    /// Determines whether the account is an admin. (Default `false`).
    ///
    /// Being admin enables to add / remove other accounts and modify permissions.
    pub is_admin: bool,

    /// Whether the `Account`'s is an rpu. (Default `false`).
    #[serde(default)]
    pub is_rpu: bool,

    /// The `Account`'s expiring date. (Default `None`).
    #[serde(default)]
    pub expire_at: Option<DateTime<Utc>>,

    /// The `Account`'s writing rights. (Default `false`).
    /// When set to `true`, the account is allowed to write into its own namespace.
    #[serde(default)]
    pub writing_rights: bool,

    /// The `Account`'s reading rights. (Default `Vec::new()`).
    #[serde(default)]
    pub reading_rights: Vec<ReadingRight>,
}
