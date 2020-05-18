//! A server for communicating between RPUs.

use crate::{batcher::Batcher, transaction_checker::TransactionChecker, BoxError};
use balise::{
    handler,
    server::{Server, TlsIdentity},
};
use prellblock_client_api::{message, ClientMessage, Pong, Transaction};
use std::{collections::HashMap, sync::Arc};
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
                GetValue(params) => self.handle_get_value(params).await,
                GetAccount(params) => self.handle_get_account(params).await,
                GetBlock(params) => self.handle_get_block(params).await,
                GetCurrentBlockNumber(params) => self.handle_get_current_block_number(params).await,
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
        let peer_id = transaction.signer();

        // Verify permissions
        self.transaction_checker
            .verify_permissions(peer_id, &transaction)?;

        match &transaction as &Transaction {
            Transaction::KeyValue { key, value } => {
                // TODO: Deserialize value.
                log::info!("Client {} set {} to {:?}.", peer_id, key, value);
            }
        }

        let batcher = self.batcher.clone();
        tokio::spawn(async move {
            batcher.add_to_batch(transaction.into()).await;
        });

        Ok(())
    }

    async fn handle_get_value(&self, params: message::GetValue) -> Response<message::GetValue> {
        let message::GetValue(peer_ids, filter, query) = params;
        let response = HashMap::new();

        // TODO: implement :D
        let _ = (peer_ids, filter, query);

        Ok(response)
    }

    async fn handle_get_account(
        &self,
        params: message::GetAccount,
    ) -> Response<message::GetAccount> {
        let message::GetAccount(peer_ids) = params;
        let response = Vec::new();

        // TODO: implement :D
        let _ = peer_ids;

        Ok(response)
    }

    async fn handle_get_block(&self, params: message::GetBlock) -> Response<message::GetBlock> {
        let message::GetBlock(filter) = params;
        let response = Vec::new();

        // TODO: implement :D
        let _ = filter;

        Ok(response)
    }

    async fn handle_get_current_block_number(
        &self,
        params: message::GetCurrentBlockNumber,
    ) -> Response<message::GetCurrentBlockNumber> {
        let message::GetCurrentBlockNumber() = params;
        let response = crate::consensus::BlockNumber::default();

        // TODO: implement :D

        Ok(response)
    }
}
