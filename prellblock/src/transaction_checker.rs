//! Module to check permissions of transactions.

use crate::world_state::{WorldState, WorldStateService};
use err_derive::Error;
use pinxit::{verify_signed_batch_iter, PeerId, Signed, VerifiedRef};
use prellblock_client_api::{
    account::{Account, AccountType, ReadingPermission},
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

    /// The account is not an admin.
    #[error(display = "The account {} is not an admin.", 0)]
    NotAnAdmin(PeerId),

    /// The signature could not be verified.
    #[error(display = "{}", 0)]
    InvalidSignature(#[error(from)] pinxit::Error),

    /// The account's expiry date has passed.
    #[error(display = "The account {} has expired.", 0)]
    AccountExpired(PeerId),

    /// The account does not have the right read blocks.
    #[error(display = "The account {} is not an allowed to read blocks.", 0)]
    CannotReadBlocks(PeerId),

    /// The account to be created already exists.
    #[error(display = "The account {} already exists.", 0)]
    AccountAlreadyExists(PeerId),
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

    /// Get an `AcccountChecker` that can be used to verify permissions of a single account.
    pub fn account_checker(&self, peer_id: PeerId) -> Result<AccountChecker, PermissionError> {
        AccountChecker::new(&self.world_state.get(), peer_id)
    }
}

/// Checks account for data access permissions.
pub struct AccountChecker {
    peer_id: PeerId,
    account: Arc<Account>,
}

impl AccountChecker {
    fn new(world_state: &WorldState, peer_id: PeerId) -> Result<Self, PermissionError> {
        if let Some(account) = world_state.accounts.get(&peer_id) {
            // Return an error if the account is expired.
            if account.expire_at.is_expired() {
                Err(PermissionError::AccountExpired(peer_id))
            } else {
                Ok(Self {
                    peer_id,
                    account: account.clone(),
                })
            }
        } else {
            Err(PermissionError::AccountNotFound(peer_id))
        }
    }

    /// This checks whether the account is allowed to read any keys of a given `peer_id`.
    #[must_use]
    pub fn is_allowed_to_read_any_key(&self, peer_id: &PeerId) -> bool {
        for reading_permission in &self.account.reading_rights {
            if let ReadingPermission::Whitelist(rights) = reading_permission {
                if rights.accounts.contains(peer_id) {
                    return true;
                }
            }
        }
        false
    }

    /// This checks whether the account is allowed to read from a given `peer_id`'s `key`.
    ///
    /// A First-Fit algorithm is used to determine the compliance of transactions to its senders permissions.
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

            // At this point we know that the permissions match the `peer_id` and `key`
            return match reading_permission {
                ReadingPermission::Whitelist(_) => true,
                ReadingPermission::Blacklist(_) => false,
            };
        }
        false
    }

    /// This checks whether the account is allowed to read with admin priviliges.
    ///
    /// This is necessary for reading account information.
    pub fn verify_is_admin(&self) -> Result<(), PermissionError> {
        if self.account.account_type == AccountType::Admin {
            Ok(())
        } else {
            Err(PermissionError::NotAnAdmin(self.peer_id.clone()))
        }
    }

    /// Verify whether the account is a known RPU.
    pub fn verify_is_rpu(&self) -> Result<(), PermissionError> {
        match self.account.account_type {
            AccountType::RPU { .. } => Ok(()),
            _ => Err(PermissionError::NotAnRPU(self.peer_id.clone())),
        }
    }

    /// Verify whether the account is allowed to read blocks.
    pub fn verify_can_read_blocks(&self) -> Result<(), PermissionError> {
        match self.account.account_type {
            AccountType::BlockReader | AccountType::RPU { .. } | AccountType::Admin => Ok(()),
            AccountType::Normal => Err(PermissionError::CannotReadBlocks(self.peer_id.clone())),
        }
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
        let account_checker = AccountChecker::new(&self.world_state, transaction.signer().clone())?;

        match &*transaction {
            Transaction::KeyValue { .. } => {
                if account_checker.account.writing_rights {
                    Ok(())
                } else {
                    Err(PermissionError::WriteDenied(account_checker.peer_id))
                }
            }
            Transaction::UpdateAccount(params) => {
                account_checker.verify_is_admin()?;
                if self.world_state.accounts.get(&params.id).is_none() {
                    return Err(PermissionError::AccountNotFound(params.id.clone()));
                }
                self.world_state
                    .apply_transaction(transaction.to_owned().into());
                Ok(())
            }
            Transaction::CreateAccount(params) => {
                account_checker.verify_is_admin()?;
                if self.world_state.accounts.get(&params.id).is_some() {
                    return Err(PermissionError::AccountAlreadyExists(params.id.clone()));
                }
                self.world_state
                    .apply_transaction(transaction.to_owned().into());
                Ok(())
            }
            Transaction::DeleteAccount(params) => {
                account_checker.verify_is_admin()?;
                if self.world_state.accounts.get(&params.id).is_none() {
                    return Err(PermissionError::AccountNotFound(params.id.clone()));
                }
                self.world_state
                    .apply_transaction(transaction.to_owned().into());
                Ok(())
            }
        }
    }
}
