//! A server for communicating between RPUs.

use crate::peer::Sender;
use balise::{
    server::{Handler, Response, Server},
    Request,
};
use prellblock_client_api::{message, ClientMessage, Pong, Transaction};
use std::net::{SocketAddr, TcpListener};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A receiver (server) instance.
///
/// # Example
///
/// ```no_run
/// use prellblock::turi::Turi;
/// use std::{net::TcpListener, sync::Arc};
///
/// let bind_addr = "127.0.0.1:0"; // replace 0 with a real port
///
/// let listener = TcpListener::bind(bind_addr).unwrap();
/// let peer_addresses = vec!["127.0.0.1:2480".parse().unwrap()]; // The ip addresses + ports of all other peers.
/// let turi = Turi::new("path_to_pfx.pfx".to_string(), peer_addresses);
/// std::thread::spawn(move || {
///     turi.serve(&listener).unwrap();
/// });
/// ```
#[derive(Clone)]
pub struct Turi {
    tls_identity: String,
    peer_addresses: Vec<SocketAddr>,
}

impl Turi {
    /// Create a new receiver instance.
    ///
    /// The `identity` is a path to a `.pfx` file.
    #[must_use]
    pub const fn new(tls_identity: String, peer_addresses: Vec<SocketAddr>) -> Self {
        Self {
            tls_identity,
            peer_addresses,
        }
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
        let tls_identity = self.tls_identity.clone();
        let server = Server::new(self, tls_identity, "prellblock")?;
        server.serve(listener)
    }

    fn handle_execute(&self, params: message::Execute) -> Result<(), BoxError> {
        let message::Execute(peer_id, transaction) = params;
        // Check validity of transaction signature.
        let transaction = transaction.verify(&peer_id)?;

        match &transaction as &Transaction {
            Transaction::KeyValue { key, value } => {
                log::info!("Client {} set {} to {}.", peer_id, key, value);
            }
        }

        let message = crate::peer::message::Execute(peer_id, transaction.into());
        let mut thread_join_handles = Vec::new();

        // Broadcast transaction to all RPUs.
        for &peer_address in &self.peer_addresses {
            let message = message.clone();
            thread_join_handles.push((
                format!("Sender ({})", peer_address),
                std::thread::spawn(move || {
                    let mut sender = Sender::new(peer_address);
                    match sender.send_request(message) {
                        Ok(()) => log::debug!("Successfully sent message to peer {}", peer_address),
                        Err(err) => {
                            log::error!("Failed sending message to peer {}: {}", peer_address, err)
                        }
                    }
                }),
            ));
        }

        for (name, join_handle) in thread_join_handles {
            match join_handle.join() {
                Err(err) => log::error!("Error occurred waiting for {}: {:?}", name, err),
                Ok(()) => log::info!("Ended {}.", name),
            };
        }
        Ok(())
    }
}

impl Handler<ClientMessage> for Turi {
    fn handle(&self, _addr: &SocketAddr, req: ClientMessage) -> Result<Response, BoxError> {
        // handle the actual request
        let res = match req {
            ClientMessage::Ping(params) => params.handle(|_| Ok(Pong)),
            ClientMessage::Execute(params) => params.handle(|params| self.handle_execute(params)),
        };
        Ok(res?)
    }
}
