//! A server for communicating between RPUs.

use balise::{
    server::{Handler, Response, Server},
    Request,
};
use prellblock_client_api::{ClientMessage, Pong};
use std::net::{SocketAddr, TcpListener};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A receiver (server) instance.
///
/// # Example
///
/// ```
/// use prellblock::turi::Turi;
/// use std::{net::TcpListener, sync::Arc};
///
/// let bind_addr = "127.0.0.1:0"; // replace 0 with a real port
///
/// let listener = TcpListener::bind(bind_addr).unwrap();
/// let turi = Turi::new("path_to_pfx.pfx".to_string());
/// std::thread::spawn(move || {
///     turi.serve(&listener).unwrap();
/// });
/// ```
#[derive(Clone)]
pub struct Turi {
    tls_identity: String,
}

impl Turi {
    /// Create a new receiver instance.
    ///
    /// The `identity` is a path to a `.pfx` file.
    #[must_use]
    pub const fn new(tls_identity: String) -> Self {
        Self { tls_identity }
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
        let tls_identity = self.tls_identity.clone();
        let server = Server::new(self, tls_identity, "prellblock")?;
        server.serve(listener)
    }
}

impl Handler<ClientMessage> for Turi {
    fn handle(&self, _addr: &SocketAddr, req: ClientMessage) -> Result<Response, BoxError> {
        // handle the actual request
        let res = match req {
            ClientMessage::Ping(params) => params.handle(|_| Pong),
        };
        Ok(res?)
    }
}
