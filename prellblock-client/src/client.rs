//! A client for communicating between RPUs.

#![allow(clippy::future_not_send)]

use balise::{client, Address, Error};
use newtype_enum::{Enum, Variant};
use pinxit::{Identity, PeerId, Signable, Signed};
use prellblock_client_api::{
    account::{Account, Permissions},
    consensus::{Block, BlockNumber},
    message, transaction, ClientMessage, Filter, GetAccount, GetBlock, GetCurrentBlockNumber,
    GetValue, Query, ReadValues, Transaction,
};
use serde::Serialize;
use std::time::SystemTime;

/// A Client Instance.
///
/// Used for Communication between Client Entities and RPU Servers.
///
/// # Example
///
/// ```no_run
/// use prellblock_client::Client;
///
/// # async fn test() -> Result<(), Box<dyn std::error::Error>> {
/// let identity = "03d738c972f37a6fd9b33278ac0c50236e45637bcd5aeee82d8323655257d256".parse()?;
/// let mut client = Client::new("10.10.10.10:2480".parse().unwrap(), identity);
/// client.send_key_value("key".to_string(), "value").await?;
/// # Ok(())
/// # }
/// ```
pub struct Client {
    rpu_client: client::Client<ClientMessage>,
    identity: Identity,
}

impl Client {
    /// Create a new client for sending transactions to an RPU.
    ///
    /// The `turi_address` is the Turi's port to connect to.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(turi_address: Address, identity: Identity) -> Self {
        Self {
            rpu_client: client::Client::new(turi_address),
            identity,
        }
    }

    fn sign<T>(&self, value: T) -> Result<Signed<T>, Error>
    where
        T: Signable,
    {
        value
            .sign(&self.identity)
            .map_err(|err| Error::BoxError(err.into()))
    }

    /// Execute a transaction.
    async fn execute<T>(&mut self, transaction: T) -> Result<(), Error>
    where
        T: Variant<Transaction> + Send,
    {
        let transaction = Transaction::from_variant(transaction);
        self.rpu_client
            .send_request(message::Execute(self.sign(transaction)?))
            .await
    }

    /// Send a key-value transaction.
    pub async fn send_key_value<V>(&mut self, key: String, value: V) -> Result<(), Error>
    where
        V: Serialize + Send,
    {
        let value = postcard::to_stdvec(&value)?;
        self.execute(transaction::KeyValue {
            key,
            value,
            timestamp: SystemTime::now(),
        })
        .await
    }

    /// Update a `target` account's `permissions`.
    pub async fn update_account(
        &mut self,
        target: PeerId,
        permissions: Permissions,
    ) -> Result<(), Error> {
        self.execute(transaction::UpdateAccount {
            id: target,
            permissions,
            timestamp: SystemTime::now(),
        })
        .await
    }

    /// Create a new account with `permissions`.
    pub async fn create_account(
        &mut self,
        account: PeerId,
        name: String,
        permissions: Permissions,
    ) -> Result<(), Error> {
        self.execute(transaction::CreateAccount {
            id: account,
            name,
            permissions,
            timestamp: SystemTime::now(),
        })
        .await
    }

    /// Delete an account.
    pub async fn delete_account(&mut self, account: PeerId) -> Result<(), Error> {
        self.execute(transaction::DeleteAccount {
            id: account,
            timestamp: SystemTime::now(),
        })
        .await
    }

    /// Query one or multiple accounts.
    ///
    /// All accounts `Accounts` matching the `peer_ids` will be returned.
    /// Nonexisting `PeerId`s will be skipped (no error).
    pub async fn query_account(&mut self, peer_ids: Vec<PeerId>) -> Result<Vec<Account>, Error> {
        let message = GetAccount { peer_ids };
        self.rpu_client
            .send_request(message::GetAccount(self.sign(message)?))
            .await
    }

    /// Query the value(s) of specific key-value pairs.
    ///
    /// If the requested data in `filter` or `query` does not exist in a account,
    /// the returned value will be empty (no error will be returned).
    ///
    /// # Example
    /// ```no_run
    /// # use prellblock_client::Client;
    /// use prellblock_client::{Filter, Query};
    ///
    /// # async fn test(client: &mut Client)  -> Result<(), Box<dyn std::error::Error>>{
    /// // Query the last 5 values skipping every second.
    /// let peer_id = "4242424242424242424242424242424242424242424242424242424242424242".parse()?;
    /// let filter = "speed".to_string();
    /// let query = Query::Range {
    ///     span: 5.into(),
    ///     end: 0.into(),
    ///     skip: Some(1.into()),
    /// };
    /// client.query_values(vec![peer_id], filter, query).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query_values(
        &mut self,
        peer_ids: Vec<PeerId>,
        filter: impl Into<Filter<String>>,
        query: Query,
    ) -> Result<ReadValues, Error> {
        let message = GetValue {
            peer_ids,
            filter: filter.into(),
            query,
        };
        self.rpu_client
            .send_request(message::GetValue(self.sign(message)?))
            .await
    }

    /// Query the current value of specific key-value pairs.
    ///
    /// # Example
    /// ```no_run
    /// # use prellblock_client::Client;
    /// # async fn test(client: &mut Client)  -> Result<(), Box<dyn std::error::Error>>{
    /// let peer_id = "4242424242424242424242424242424242424242424242424242424242424242".parse()?;
    /// // Filter all values being lexicographically sorted between "speed" and "z"
    /// let filter = "speed".to_string().."z".to_string();
    /// client.query_current_value(vec![peer_id], filter).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query_current_value(
        &mut self,
        peer_ids: Vec<PeerId>,
        filter: impl Into<Filter<String>>,
    ) -> Result<ReadValues, Error> {
        self.query_values(peer_ids, filter, Query::CurrentValue)
            .await
    }

    /// Retrieve blocks from the chain.
    ///
    /// Nonexisting blocks specified by the `filter` will be ignored (no error will be returned).
    ///
    /// # Example
    /// ```no_run
    /// # use prellblock_client::Client;
    /// use prellblock_client::consensus::BlockNumber;
    ///
    /// # async fn test(client: &mut Client)  -> Result<(), Box<dyn std::error::Error>>{
    /// let filter = BlockNumber::new(0)..BlockNumber::new(42);
    /// client.query_block(filter).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query_block(
        &mut self,
        filter: impl Into<Filter<BlockNumber>>,
    ) -> Result<Vec<Block>, Error> {
        let message = GetBlock {
            filter: filter.into(),
        };
        self.rpu_client
            .send_request(message::GetBlock(self.sign(message)?))
            .await
    }

    /// Retrieve the current block number.
    ///
    /// # Example
    /// ```no_run
    /// # use prellblock_client::Client;
    /// # async fn test(client: &mut Client)  -> Result<(), Box<dyn std::error::Error>>{
    /// let block_number = client.current_block_number().await?;
    /// println!("Current block in chain is: {:?}", block_number);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn current_block_number(&mut self) -> Result<BlockNumber, Error> {
        self.rpu_client
            .send_request(message::GetCurrentBlockNumber(
                self.sign(GetCurrentBlockNumber)?,
            ))
            .await
    }
}
