//! A client for communicating between RPUs.

use balise::{client, Error};
use newtype_enum::{Enum, Variant};
use pinxit::{Identity, PeerId, Signable};
use prellblock_client_api::{
    account_permissions::Permissions, message, transaction, ClientMessage, Transaction,
};
use serde::Serialize;
use std::net::SocketAddr;

/// A Client Instance.
///
/// Used for Communication between Client Entities and RPU Servers.
///
/// # Example
///
/// ```no_run
/// use pinxit::Identity;
/// use prellblock_client::Client;
///
/// # async fn test() {
/// let identity: Identity = "03d738c972f37a6fd9b33278ac0c50236e45637bcd5aeee82d8323655257d256"
///     .parse()
///     .unwrap();
/// let mut client = Client::new("10.10.10.10:2480".parse().unwrap(), identity);
/// client
///     .send_key_value("key".to_string(), "value")
///     .await
///     .unwrap();
/// # }
/// ```
pub struct Client {
    rpu_client: client::Client<ClientMessage>,
    identity: Identity,
}

impl Client {
    /// Create a new client for sending transactions to a RPU.
    ///
    /// The `turi_address` is the Turi's port to connect to.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(turi_address: SocketAddr, identity: Identity) -> Self {
        Self {
            rpu_client: client::Client::new(turi_address),
            identity,
        }
    }

    /// Execute a transaction.Transaction
    async fn execute<T>(&mut self, transaction: T) -> Result<(), Error>
    where
        T: Variant<Transaction> + Send,
    {
        let transaction = Transaction::from_variant(transaction)
            .sign(&self.identity)
            .map_err(|err| Error::BoxError(err.into()))?;

        self.rpu_client
            .send_request(message::Execute(transaction))
            .await
    }

    /// Send a key-value transaction.
    pub async fn send_key_value<V>(&mut self, key: String, value: V) -> Result<(), Error>
    where
        V: Serialize + Send,
    {
        let value = postcard::to_stdvec(&value)?;
        self.execute(transaction::KeyValue { key, value }).await
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
        })
        .await
    }
}
