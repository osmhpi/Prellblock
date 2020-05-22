//! A client for communicating between RPUs.

use balise::{client, Error};
use newtype_enum::Enum;
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

    /// Send a key-value transaction.
    pub async fn send_key_value<V>(&mut self, key: String, value: V) -> Result<(), Error>
    where
        V: AsRef<[u8]> + Serialize + Send,
    {
        let value = postcard::to_stdvec(&value).unwrap();

        let transaction = Transaction::from_variant(transaction::KeyValue { key, value })
            .sign(&self.identity)
            .unwrap();

        self.rpu_client
            .send_request(message::Execute(transaction))
            .await
    }

    /// Update a `target` account's `permissions`.
    pub async fn update_account(
        &mut self,
        target: PeerId,
        permissions: Permissions,
    ) -> Result<(), Error> {
        let transaction = Transaction::from_variant(transaction::UpdateAccount {
            id: target,
            permissions,
        })
        .sign(&self.identity)
        .unwrap();
        self.rpu_client
            .send_request(message::Execute(transaction))
            .await
    }
}
