//! A server for communicating between RPUs.

use crate::{batcher::Batcher, reader::Reader, transaction_checker::TransactionChecker, BoxError};
use balise::{
    handler,
    server::{Server, TlsIdentity},
};
use prellblock_client_api::{message, ClientMessage, Pong, Transaction};
use std::sync::Arc;
use tokio::net::TcpListener;

type Response<R> = Result<<R as balise::Request<ClientMessage>>::Response, BoxError>;

/// A receiver (server) instance.
///
/// The Turi (old German for "door") is the entrypoint for
/// any clients to send transactions.
#[derive(Clone)]
pub struct Turi {
    tls_identity: TlsIdentity,
    batcher: Arc<Batcher>,
    reader: Reader,
    transaction_checker: TransactionChecker,
}

#[cfg(feature = "monitoring")]
use lazy_static::lazy_static;
#[cfg(feature = "monitoring")]
use prometheus::{labels, opts, register_int_counter, IntCounter};
#[cfg(feature = "monitoring")]
lazy_static! {
    static ref TURI_RECEIVED_TRANSACTIONS: IntCounter = register_int_counter!(opts! {
        "turi_num_received_txs",
        "Number of transactions received by the Turi.",
        labels! {
            "name" => "emily",
        }
    })
    .unwrap();
    static ref TURI_RECEIVED_INVALID_TRANSACTIONS: IntCounter = register_int_counter!(
        "turi_num_received_invalid_txs",
        "Number of invalid transactions received by the Turi."
    )
    .unwrap();
}

impl Turi {
    /// Create a new receiver instance.
    ///
    /// The `identity` is a path to a `.pfx` file.
    #[must_use]
    pub const fn new(
        tls_identity: TlsIdentity,
        batcher: Arc<Batcher>,
        reader: Reader,
        transaction_checker: TransactionChecker,
    ) -> Self {
        Self {
            tls_identity,
            batcher,
            reader,
            transaction_checker,
        }
    }

    /// The main server loop.
    pub async fn serve(self, listener: &mut TcpListener) -> Result<(), balise::Error> {
        let tls_identity = self.tls_identity.clone();
        let server = Server::new(
            handler!(ClientMessage, {
                Ping(_) => Ok(Pong),
                Execute(params) => {
                    #[cfg(feature = "monitoring")]
                    TURI_RECEIVED_TRANSACTIONS.inc();

                    let result = self.handle_execute(params).await;
                    #[cfg(feature = "monitoring")]
                    if result.is_err() {
                        TURI_RECEIVED_INVALID_TRANSACTIONS.inc();
                    }

                    result
                },
                GetValue(params) => self.reader.handle_get_value(params).await,
                GetAccount(params) => self.reader.handle_get_account(params).await,
                GetBlock(params) => self.reader.handle_get_block(params).await,
                GetCurrentBlockNumber(params) => self.reader.handle_get_current_block_number(params).await,
            }),
            tls_identity,
        )?;
        server.serve(listener).await?;
        Ok(())
    }

    async fn handle_execute(&self, params: message::Execute) -> Response<message::Execute> {
        let message::Execute(transaction) = params;

        // Check validity of transaction signature.
        let transaction = transaction.verify()?;

        // Verify permissions
        self.transaction_checker
            .verify_permissions(transaction.borrow())?;

        let peer_id = transaction.signer();
        match &*transaction {
            Transaction::KeyValue(params) => {
                // TODO: Deserialize value.
                log::debug!(
                    "Client {} set {} to {:?} (time since transaction-creation on the client: {:?}).",
                    peer_id,
                    params.key,
                    params.value,
                    std::time::SystemTime::now().duration_since(params.timestamp),
                );
            }
            Transaction::UpdateAccount(params) => {
                log::debug!(
                    "Client {} updates account {}: {:#?} (time since transaction-creation on the client: {:?}).",
                    &transaction.signer(),
                    params.id,
                    params.permissions,
                    std::time::SystemTime::now().duration_since(params.timestamp),
                );
            }
            Transaction::CreateAccount(params) => {
                log::debug!(
                    "Client {} creates account {}: {:#?} (time since transaction-creation on the client: {:?}).",
                    &transaction.signer(),
                    params.id,
                    params.permissions,
                    std::time::SystemTime::now().duration_since(params.timestamp),
                );
            }
            Transaction::DeleteAccount(params) => {
                log::debug!(
                    "Client {} deletes account {} (time since transaction-creation on the client: {:?}).",
                    &transaction.signer(),
                    params.id,
                    std::time::SystemTime::now().duration_since(params.timestamp),
                );
            }
        }

        let batcher = self.batcher.clone();
        tokio::spawn(async move {
            batcher.add_to_batch(transaction.into()).await;
        });

        Ok(())
    }
}
