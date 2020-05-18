//! Module to check permissions of transactions.

use crate::world_state::{WorldState, WorldStateService};
use err_derive::Error;
use pinxit::{verify_signed_batch_ref, PeerId, Signed, VerifiedRef};
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

    /// The account *setting* the permissions is not permitted to do so.
    #[error(
        display = "The account {} is no permitted to change permissions of other accounts.",
        0
    )]
    PermissionChangeDenied(PeerId),
}

/// A `TransactionChecker` is used to check whether accounts are allowed to carry out transactions.
#[derive(Debug)]
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
        transaction: VerifiedRef<Transaction>,
    ) -> Result<(), PermissionError> {
        let mut temporary_world_state = self.world_state.get();
        verify_permissions_and_apply(peer_id, transaction, &mut temporary_world_state)
    }

    /// Verify signatures of `Transaction`s
    pub fn verify(&self, data: &[Signed<Transaction>]) -> Result<(), PermissionError> {
        let verified_transactions = verify_signed_batch_ref(data)?;
        let mut temporary_world_state = self.world_state.get();
        for tx in verified_transactions {
            verify_permissions_and_apply(tx.signer(), tx, &mut temporary_world_state)?;
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

/// Verify whether a given `transaction` issued by a `peer_id` is valid.
///
/// This also applies the `transaction` to the `world_state`.
/// Provide a temporary copy that will be dropped if you do not want this to have an effect.
fn verify_permissions_and_apply(
    peer_id: &PeerId,
    transaction: VerifiedRef<Transaction>,
    world_state: &mut WorldState,
) -> Result<(), PermissionError> {
    match &*transaction {
        Transaction::KeyValue { .. } => {
            if let Some(account) = world_state.accounts.get(peer_id) {
                if account.writing_rights {
                    Ok(())
                } else {
                    Err(PermissionError::WriteDenied(peer_id.clone()))
                }
            } else {
                Err(PermissionError::AccountNotFound(peer_id.clone()))
            }
        }
        Transaction::UpdateAccount(params) => {
            if let Some(account) = world_state.accounts.get(peer_id) {
                if account.is_admin {
                    if world_state.accounts.get(&params.id).is_none() {
                        return Err(PermissionError::AccountNotFound(params.id.clone()));
                    }
                    world_state.apply_transaction(transaction.to_owned().into());
                    Ok(())
                } else {
                    Err(PermissionError::PermissionChangeDenied(peer_id.clone()))
                }
            } else {
                Err(PermissionError::AccountNotFound(peer_id.clone()))
            }
        }
    }
}
