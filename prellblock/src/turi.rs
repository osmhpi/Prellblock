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
/// use prellblock::peer::{Calculator, Receiver};
/// use std::{net::TcpListener, sync::Arc};
///
/// let calculator = Calculator::new();
/// let calculator = Arc::new(calculator.into());
/// let bind_addr = "127.0.0.1:1234";
///
/// let listener = TcpListener::bind(bind_addr).unwrap();
/// let receiver = Receiver::new(calculator);
/// std::thread::spawn(move || {
///     receiver.serve(&listener).unwrap();
/// });
/// ```
#[derive(Clone)]
pub struct Turi {}

impl Turi {
    /// Create a new receiver instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {}
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
        let server = Server::new(self);
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
