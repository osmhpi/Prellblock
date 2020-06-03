//! A server for communicating between RPUs.

use crate::{
    block_storage::BlockStorage, transaction_checker::TransactionChecker,
    world_state::WorldStateService, BoxError,
};
use prellblock_client_api::{message, ClientMessage};

type Response<R> = Result<<R as balise::Request<ClientMessage>>::Response, BoxError>;

/// The `Reader` component responds to read queries.
#[derive(Clone)]
pub struct Reader {
    block_storage: BlockStorage,
    world_state: WorldStateService,
    transaction_checker: TransactionChecker,
}

impl Reader {
    /// Create a new reader instance.
    #[must_use]
    pub fn new(block_storage: BlockStorage, world_state: WorldStateService) -> Self {
        Self {
            block_storage,
            world_state: world_state.clone(),
            transaction_checker: TransactionChecker::new(world_state),
        }
    }

    pub(crate) async fn handle_get_value(
        &self,
        params: message::GetValue,
    ) -> Response<message::GetValue> {
        let message::GetValue(message) = params;
        let message = message.verify()?;

        let account_checker = self
            .transaction_checker
            .account_checker(message.signer().clone())?;

        let message = message.into_inner();
        let filter = message.filter;
        let query = message.query;

        #[allow(clippy::filter_map)]
        message
            .peer_ids
            .into_iter()
            .filter(|peer_id| account_checker.is_allowed_to_read_any_key(peer_id))
            .map(|peer_id| {
                let transactions = self.block_storage.read_transactions(
                    &account_checker,
                    &peer_id,
                    filter.as_deref(),
                    &query,
                )?;
                Ok((peer_id, transactions))
            })
            .collect()
    }

    pub(crate) async fn handle_get_account(
        &self,
        params: message::GetAccount,
    ) -> Response<message::GetAccount> {
        let message::GetAccount(message) = params;
        let message = message.verify()?;

        self.transaction_checker
            .account_checker(message.signer().clone())?
            .verify_is_admin()?;

        let world_state = self.world_state.get();
        let accounts = message
            .peer_ids
            .iter()
            .filter_map(|peer_id| {
                world_state
                    .accounts
                    .get(peer_id)
                    .map(|account| (**account).clone())
            })
            .collect();

        Ok(accounts)
    }

    pub(crate) async fn handle_get_block(
        &self,
        params: message::GetBlock,
    ) -> Response<message::GetBlock> {
        let message::GetBlock(message) = params;
        let message = message.verify()?;

        self.transaction_checker
            .account_checker(message.signer().clone())?
            .verify_can_read_blocks()?;

        let message = message.into_inner();
        let blocks: Result<_, _> = self.block_storage.read(message.filter).collect();

        Ok(blocks?)
    }

    /// The function will return the current blocknumber,
    /// as long as the issuer has a valid account.
    pub(crate) async fn handle_get_current_block_number(
        &self,
        params: message::GetCurrentBlockNumber,
    ) -> Response<message::GetCurrentBlockNumber> {
        let message::GetCurrentBlockNumber(message) = params;
        let message = message.verify()?;

        // The sender needs to have a valid account.
        self.transaction_checker
            .account_checker(message.signer().clone())?;

        let world_state = self.world_state.get();
        let block_number = world_state.block_number;

        Ok(block_number)
    }
}
