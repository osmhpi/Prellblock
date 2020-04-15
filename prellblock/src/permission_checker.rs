//! Module to check permissions of transactions.

use crate::world_state::WorldState;
use err_derive::Error;
use pinxit::PeerId;
use prellblock_client_api::Transaction;

/// An error of the `pinxit` crate.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PermissionError {
    /// A transaction tried to write a value, but the account is not allowd to write data.
    #[error(display = "The account {} is not allowed to write", 0)]
    WriteDenied(PeerId),

    /// The account was not found.
    #[error(display = "The account {} was not found", 0)]
    AccountNotFound(PeerId),
}

/// A `PermissionChecker` is used to check whether accounts are allowed to carry out transactions.
pub struct PermissionChecker {
    world_state: WorldState,
}

impl PermissionChecker {
    /// Create a new instance of `PermissionChecker`.
    #[must_use]
    pub const fn new(world_state: WorldState) -> Self {
        Self { world_state }
    }

    /// Verify whether a given `transaction` issued by a `peer_id` is valid.
    pub fn verify(
        &self,
        peer_id: &PeerId,
        transaction: &Transaction,
    ) -> Result<(), PermissionError> {
        match transaction {
            Transaction::KeyValue { .. } => {
                if let Some(account) = self.world_state.accounts.get(peer_id) {
                    if account.writing_rights {
                        Ok(())
                    } else {
                        Err(PermissionError::WriteDenied(peer_id.clone()))
                    }
                } else {
                    Err(PermissionError::AccountNotFound(peer_id.clone()))
                }
            }
        }
    }
}
