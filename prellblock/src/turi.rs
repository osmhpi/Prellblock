//! A server for communicating between RPUs.

use crate::{batcher::Batcher, BoxError};
use balise::{
    handle_fn,
    server::{Handler, Server, TlsIdentity},
    Request,
};
use prellblock_client_api::{message, ClientMessage, Pong, Transaction};
use std::{net::TcpListener, sync::Arc};

/// A receiver (server) instance.
///
/// The Turi (old German for "door") is the entrypoint for
/// any clients to send transactions.
#[derive(Clone)]
pub struct Turi {
    tls_identity: TlsIdentity,
    batcher: Arc<Batcher>,
}

impl Turi {
    /// Create a new receiver instance.
    ///
    /// The `identity` is a path to a `.pfx` file.
    #[must_use]
    pub const fn new(tls_identity: TlsIdentity, batcher: Arc<Batcher>) -> Self {
        Self {
            tls_identity,
            batcher,
        }
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
        let tls_identity = self.tls_identity.clone();
        let server = Server::new(self, tls_identity)?;
        server.serve(listener)
    }

    fn handle_execute(&self, params: message::Execute) -> Result<(), BoxError> {
        let message::Execute(transaction) = params;
        // Check validity of transaction signature.
        let transaction = transaction.verify()?;
        let peer_id = transaction.signer();
        match &transaction as &Transaction {
            Transaction::KeyValue { key, value } => {
                log::info!("Client {} set {} to {}.", peer_id, key, value);
            }
        }

        self.batcher.clone().add_to_batch(transaction.into());

        Ok(())
    }
}

impl Handler<ClientMessage> for Turi {
    handle_fn!(self, ClientMessage, {
        Ping(_) => Ok(Pong),
        Execute(params) => self.handle_execute(params),
    });
}
