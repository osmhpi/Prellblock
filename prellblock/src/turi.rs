//! A server for communicating between RPUs.

use crate::{batcher::Batcher, permission_checker::PermissionChecker, BoxError};
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
    permission_checker: Arc<PermissionChecker>,
}

impl Turi {
    /// Create a new receiver instance.
    ///
    /// The `identity` is a path to a `.pfx` file.
    #[must_use]
    pub const fn new(
        tls_identity: TlsIdentity,
        batcher: Arc<Batcher>,
        permission_checker: Arc<PermissionChecker>,
    ) -> Self {
        Self {
            tls_identity,
            batcher,
            permission_checker,
        }
    }

    /// The main server loop.
    pub async fn serve(self, listener: &mut TcpListener) -> Result<(), BoxError> {
        let tls_identity = self.tls_identity.clone();
        let server = Server::new(
            handler!(ClientMessage, {
                Ping(_) => Ok(Pong),
                Execute(params) => self.handle_execute(params).await,
            }),
            tls_identity,
        )?;
        server.serve(listener).await
    }

    async fn handle_execute(&self, params: message::Execute) -> Result<(), BoxError> {
        let message::Execute(transaction) = params;
        // Check validity of transaction signature.
        let transaction = transaction.verify()?;
        let peer_id = transaction.signer();

        // Verify permissions
        self.permission_checker.verify(peer_id, &transaction)?;

        match &transaction as &Transaction {
            Transaction::KeyValue { key, value } => {
                // TODO: Deserialize value.
                log::info!("Client {} set {} to {:?}.", peer_id, key, value);
            }
        }

        self.batcher.clone().add_to_batch(transaction.into()).await;

        Ok(())
    }
}
