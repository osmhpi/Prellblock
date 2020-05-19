use super::{Error, Follower, InvalidTransaction};
use pinxit::{verify_signed_batch_iter, Signed};
use prellblock_client_api::Transaction;

impl Follower {
    /// Stateful validate transactions sent by the leader.
    pub(super) fn stateful_validate(
        &self,
        valid_transactions: &[Signed<Transaction>],
        invalid_transactions: &[InvalidTransaction],
    ) -> Result<(), Error> {
        let number_of_valid_transactions = valid_transactions.len();
        let mut valid_transactions = verify_signed_batch_iter(valid_transactions.iter())?;

        let invalid_transactions_iter = invalid_transactions
            .iter()
            .map(|(_, transaction)| transaction);

        // The order of the verified (invalid) transactions is the same!
        // Zipping with the index should be ok
        let mut invalid_transactions = invalid_transactions
            .iter()
            .map(|(index, _)| index)
            .zip(verify_signed_batch_iter(invalid_transactions_iter)?);

        let mut check = self.transaction_checker.check();

        let mut index = 0;
        loop {
            let invalid_item = invalid_transactions.next();

            // apply all transactions declared as valid until reaching a invalid one
            let end_index = match invalid_item {
                Some((index, _)) => *index,
                None => number_of_valid_transactions,
            };
            while index < end_index {
                if let Some(tx) = valid_transactions.next() {
                    check.verify_permissions_and_apply(tx)?;
                    index += 1;
                } else {
                    return Err(Error::BadInvalidTransactionIndex(index));
                }
            }

            // Applying the transaction marked as invalid should fail!
            // Otherwise the leader tries to trick followers into dropping valid transactions
            // from the queue (which is like censorship).
            if let Some((_, verified_invalid_transaction)) = invalid_item {
                if check
                    .verify_permissions_and_apply(verified_invalid_transaction)
                    .is_ok()
                {
                    return Err(Error::CensorshipDetected(
                        (*verified_invalid_transaction).clone().into(),
                    ));
                }
            } else {
                break;
            }
        }

        // All transactions should be applied by now.
        assert_eq!(valid_transactions.len(), 0);
        assert_eq!(invalid_transactions.len(), 0);

        Ok(())
    }
}
