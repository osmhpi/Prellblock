//! Module to check permissions of transactions.

use crate::world_state::WorldStateService;
use err_derive::Error;
use pinxit::{PeerId, Signed};
use prellblock_client_api::Transaction;

/// An error of the `permission_checker` module.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PermissionError {
    /// A transaction tried to write a value, but the account is not allowd to write data.
    #[error(display = "The account {} is not allowed to write.", 0)]
    WriteDenied(PeerId),

    /// The account was not found.
    #[error(display = "The account {} was not found.", 0)]
    AccountNotFound(PeerId),

    /// The account is not an RPU.
    #[error(display = "The account {} is not an RPU.", 0)]
    NotAnRPU(PeerId),

    /// The signature could not be verified.
    #[error(display = "{}", 0)]
    InvalidSignature(#[error(from)] pinxit::Error),
}

/// A `TransactionChecker` is used to check whether accounts are allowed to carry out transactions.
pub struct TransactionChecker {
    world_state: WorldStateService,
}

impl TransactionChecker {
    /// Create a new instance of `TransactionChecker`.
    #[must_use]
    pub const fn new(world_state: WorldStateService) -> Self {
        Self { world_state }
    }

    /// Verify whether a given `transaction` issued by a `peer_id` is valid.
    pub fn verify_permissions(
        &self,
        peer_id: &PeerId,
        transaction: &Transaction,
    ) -> Result<(), PermissionError> {
        match transaction {
            Transaction::KeyValue { .. } => {
                if let Some(account) = self.world_state.get().accounts.get(peer_id) {
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

    /// Verify signatures of `Transaction`s
    pub fn verify_signatures(&self, data: &[Signed<Transaction>]) -> Result<(), PermissionError> {
        for tx in data {
            let signer = tx.signer().clone();
            let transaction = tx.verify_ref()?;
            self.verify_permissions(&signer, &transaction)?;
        }
        Ok(())
    }

    /// Verify whether the given `PeerId` is a known RPU.
    pub fn verify_is_rpu(&self, peer_id: &PeerId) -> Result<(), PermissionError> {
        if let Some(account) = self.world_state.get().accounts.get(peer_id) {
            if account.is_rpu {
                Ok(())
            } else {
                Err(PermissionError::NotAnRPU(peer_id.clone()))
            }
        } else {
            Err(PermissionError::AccountNotFound(peer_id.clone()))
        }
    }
}
