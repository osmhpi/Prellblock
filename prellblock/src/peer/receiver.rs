//! A server for communicating between RPUs.

use std::{env, net::TcpListener, sync::Arc};

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
///     permission_checker::PermissionChecker,
///     world_state::WorldState,
/// };
/// use std::{net::TcpListener, sync::Arc};
///
/// let calculator = Calculator::new();
/// let calculator = Arc::new(calculator.into());
///
/// let data_storage = DataStorage::new("/tmp/some_db").unwrap();
/// let data_storage = Arc::new(data_storage);
///
/// let world_state = WorldState::default();
/// let permission_checker = PermissionChecker::new(world_state);
/// let permission_checker = Arc::new(permission_checker);
///
/// let peer_inbox = PeerInbox::new(calculator, data_storage, permission_checker);
/// let peer_inbox = Arc::new(peer_inbox);
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
    peer_inbox: Arc<PeerInbox>,
}

impl Receiver {
    /// Create a new receiver instance.
    #[must_use]
    pub const fn new(tls_identity: String, peer_inbox: Arc<PeerInbox>) -> Self {
        Self {
            tls_identity,
            peer_inbox,
        }
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
        let tls_identity = self.tls_identity.clone();
        let password = env::var(crate::TLS_PASSWORD_ENV)
            .unwrap_or_else(|_| crate::TLS_DEFAULT_PASSWORD.to_string());
        let server = Server::new(self, tls_identity, &password)?;
        drop(password);
        server.serve(listener)
    }
}

impl Handler<PeerMessage> for Receiver {
    handle_fn!(self, PeerMessage, {
        Add(params) =>  self.peer_inbox.handle_add(&params),
        Sub(params) =>  self.peer_inbox.handle_sub(&params),
        Ping(_) => self.peer_inbox.handle_ping(),
        Execute(params) => self.peer_inbox.handle_execute(params),
        ExecuteBatch(params) => self.peer_inbox.handle_execute_batch(params),    });
}
