//! A server for communicating between RPUs.

use std::net::TcpListener;

use super::{PeerInbox, PeerMessage};
use balise::{
    handle_fn,
    server::{Handler, Server},
    Request,
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A receiver (server) instance.
///
/// # Example
///
/// ```
/// use prellblock::{
///     data_storage::DataStorage,
///     peer::{Calculator, PeerInbox, Receiver},
/// };
/// use std::{net::TcpListener, sync::Arc};
///
/// let calculator = Calculator::new();
/// let calculator = Arc::new(calculator.into());
///
/// let data_storage = DataStorage::new("/tmp/some_db").unwrap();
/// let data_storage = Arc::new(data_storage);
///
/// let peer_inbox = PeerInbox::new(calculator, data_storage);
///
/// let bind_addr = "127.0.0.1:0"; // replace 0 with a real port
///
/// let listener = TcpListener::bind(bind_addr).unwrap();
/// let receiver = Receiver::new("path_to_pfx.pfx".to_string(), peer_inbox);
/// std::thread::spawn(move || {
///     receiver.serve(&listener).unwrap();
/// });
/// ```
#[derive(Clone)]
pub struct Receiver {
    tls_identity: String,
    peer_inbox: PeerInbox,
}

impl Receiver {
    /// Create a new receiver instance.
    #[must_use]
    pub const fn new(tls_identity: String, peer_inbox: PeerInbox) -> Self {
        Self {
            tls_identity,
            peer_inbox,
        }
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
        let tls_identity = self.tls_identity.clone();
        let server = Server::new(self, tls_identity, "prellblock")?;
        server.serve(listener)
    }
}

// TODO: macro?
impl Handler<PeerMessage> for Receiver {
    handle_fn!(self, PeerMessage, {
        Add(params) =>  self.peer_inbox.handle_add(&params),//calculator.lock().unwrap().add(params.0, params.1)),
        Sub(params) =>  self.peer_inbox.handle_sub(&params),//calculator.lock().unwrap().sub(params.0, params.1)),
        Ping(_) => self.peer_inbox.handle_ping(), //Ok(Pong),
        Execute(params) => self.peer_inbox.handle_execute(params),
    });
}
