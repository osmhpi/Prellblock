//! A server for communicating between RPUs.

use crate::{batcher::Batcher, transaction_checker::TransactionChecker, BoxError};
use balise::{
    handler,
    server::{Server, TlsIdentity},
};
use prellblock_client_api::{message, ClientMessage, Pong, Transaction};
use std::sync::Arc;
use tokio::net::TcpListener;

/// A receiver (server) instance.
///
/// The Turi (old German for "door") is the entrypoint for
/// any clients to send transactions.
#[derive(Clone)]
pub struct Turi {
    tls_identity: TlsIdentity,
    batcher: Arc<Batcher>,
    transaction_checker: Arc<TransactionChecker>,
}

impl Turi {
    /// Create a new receiver instance.
    ///
    /// The `identity` is a path to a `.pfx` file.
    #[must_use]
    pub const fn new(
        tls_identity: TlsIdentity,
        batcher: Arc<Batcher>,
        transaction_checker: Arc<TransactionChecker>,
    ) -> Self {
        Self {
            tls_identity,
            batcher,
            transaction_checker,
        }
    }

    /// The main server loop.
    pub async fn serve(self, listener: &mut TcpListener) -> Result<(), balise::Error> {
        let tls_identity = self.tls_identity.clone();
        let server = Server::new(
            handler!(ClientMessage, {
                Ping(_) => Ok(Pong),
                Execute(params) => self.handle_execute(params).await,
            }),
            tls_identity,
        )?;
        server.serve(listener).await?;
        Ok(())
    }

    async fn handle_execute(&self, params: message::Execute) -> Result<(), BoxError> {
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
                    "Client {} set {} to {:?}.",
                    peer_id,
                    params.key,
                    params.value,
                );
            }
            Transaction::UpdateAccount(params) => {
                log::debug!(
                    "Client {} updates account {}: {:#?}",
                    &transaction.signer(),
                    params.id,
                    params.permissions,
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
