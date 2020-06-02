//! Module to check permissions of transactions.

use crate::world_state::{WorldState, WorldStateService};
use err_derive::Error;
use pinxit::{verify_signed_batch_iter, PeerId, Signed, VerifiedRef};
use prellblock_client_api::{
    account::{Account, ReadingPermission},
    Transaction,
};
use std::sync::Arc;

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

    /// The account's expiry date has passed.
    #[error(
        display = "The account {} has expired. All transaction from this account will be denied.",
        0
    )]
    AccountExpired(PeerId),
}

/// A `TransactionChecker` is used to check whether accounts are allowed to carry out transactions.
#[derive(Debug, Clone)]
pub struct TransactionChecker {
    world_state: WorldStateService,
}

impl TransactionChecker {
    /// Create a new instance of `TransactionChecker`.
    #[must_use]
    pub const fn new(world_state: WorldStateService) -> Self {
        Self { world_state }
    }

    /// Returns a `TransactionCheck` with the current world state as virtual clone.
    #[must_use]
    pub fn check(&self) -> TransactionCheck {
        TransactionCheck {
            world_state: self.world_state.get(),
        }
    }

    /// Verify whether a given `transaction` issued by a `peer_id` is valid.
    pub fn verify_permissions(
        &self,
        transaction: VerifiedRef<Transaction>,
    ) -> Result<(), PermissionError> {
        self.check().verify_permissions_and_apply(transaction)
    }

    /// Verify signatures of `Transaction`s
    pub fn verify(&self, data: &[Signed<Transaction>]) -> Result<(), PermissionError> {
        let verified_transactions = verify_signed_batch_iter(data.iter())?;
        let mut check = self.check();
        for tx in verified_transactions {
            check.verify_permissions_and_apply(tx)?;
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

    /// Get an `AcccountChecker` that can be used to verify permissions of a single account.
    pub fn account_checker(&self, peer_id: &PeerId) -> Result<AccountChecker, PermissionError> {
        if let Some(account) = self.world_state.get().accounts.get(peer_id) {
            Ok(AccountChecker {
                account: account.clone(),
            })
        } else {
            Err(PermissionError::AccountNotFound(peer_id.clone()))
        }
    }
}

/// Filters the Request for permitted sections.
pub struct AccountChecker {
    account: Arc<Account>,
}

impl AccountChecker {
    /// This checks whether the account is allowed to read any keys of a given `peer_id`.
    #[must_use]
    pub fn is_allowed_to_read_any_key(&self, peer_id: &PeerId) -> bool {
        for reading_permission in &self.account.reading_rights {
            match reading_permission {
                ReadingPermission::Whitelist(rights) => {
                    if rights.accounts.contains(peer_id) {
                        return true;
                    }
                }
                ReadingPermission::Blacklist(_) => {}
            }
        }
        false
    }

    /// This checks whether the account is allowed to read from a given `peer_id`'s `key`.
    #[must_use]
    pub fn is_allowed_to_read_key(&self, peer_id: &PeerId, key: &str) -> bool {
        for reading_permission in &self.account.reading_rights {
            match reading_permission {
                ReadingPermission::Whitelist(rights) | ReadingPermission::Blacklist(rights) => {
                    if !rights.accounts.contains(peer_id)
                        || !rights
                            .namespace
                            .iter()
                            .any(|permission| permission.scope == key)
                    {
                        continue;
                    }
                }
            }

            // At this point we know that the rights match the `peer_id` and `key`
            return match reading_permission {
                ReadingPermission::Whitelist(_) => true,
                ReadingPermission::Blacklist(_) => false,
            };
        }
        false
    }
}

/// Helps verifying transactions statefully on a virtual `WorldState`.
#[derive(Debug)]
pub struct TransactionCheck {
    world_state: WorldState,
}

impl TransactionCheck {
    /// Verify whether a given `transaction` issued by a `peer_id` is valid.
    ///
    /// This also applies the `transaction` to the `world_state`.
    /// Provide a temporary copy that will be dropped if you do not want this to have an effect.
    pub fn verify_permissions_and_apply(
        &mut self,
        transaction: VerifiedRef<Transaction>,
    ) -> Result<(), PermissionError> {
        let signer = transaction.signer();
        if let Some(account) = self.world_state.accounts.get(signer) {
            // If a account is expired, *all* transactions will be denied.
            if account.expire_at.is_expired() {
                return Err(PermissionError::AccountExpired(signer.clone()));
            }

            match &*transaction {
                Transaction::KeyValue { .. } => {
                    if account.writing_rights {
                        Ok(())
                    } else {
                        Err(PermissionError::WriteDenied(signer.clone()))
                    }
                }
                Transaction::UpdateAccount(params) => {
                    if account.is_admin {
                        if self.world_state.accounts.get(&params.id).is_none() {
                            return Err(PermissionError::AccountNotFound(params.id.clone()));
                        }
                        self.world_state
                            .apply_transaction(transaction.to_owned().into());
                        Ok(())
                    } else {
                        Err(PermissionError::PermissionChangeDenied(signer.clone()))
                    }
                }
            }
        } else {
            Err(PermissionError::AccountNotFound(signer.clone()))
        }
    }
}
