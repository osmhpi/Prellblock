//! A server for communicating between RPUs.

use super::{PeerInbox, PeerMessage};
use crate::BoxError;
use balise::{
    handle_fn,
    server::{Handler, Server, TlsIdentity},
    Request,
};

use std::{net::TcpListener, sync::Arc};

/// A receiver (server) instance.
///
/// The `Receiver` is used to receive messages being sent between RPUs.
#[derive(Clone)]
pub struct Receiver {
    tls_identity: TlsIdentity,
    peer_inbox: Arc<PeerInbox>,
}

impl Receiver {
    /// Create a new receiver instance.
    #[must_use]
    pub const fn new(tls_identity: TlsIdentity, peer_inbox: Arc<PeerInbox>) -> Self {
        Self {
            tls_identity,
            peer_inbox,
        }
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
        let tls_identity = self.tls_identity.clone();
        let server = Server::new(self, tls_identity)?;
        server.serve(listener)
    }
}

impl Handler<PeerMessage> for Receiver {
    handle_fn!(self, PeerMessage, {
        Add(params) =>  self.peer_inbox.handle_add(&params),
        Sub(params) =>  self.peer_inbox.handle_sub(&params),
        Ping(_) => self.peer_inbox.handle_ping(),
        ExecuteBatch(params) => self.peer_inbox.handle_execute_batch(params),
        Consensus(params) => self.peer_inbox.handle_consensus(params),
    });
}
