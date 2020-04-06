//! A server for communicating between RPUs.

use balise::{
    server::{Handler, Server},
    Request,
};
use client_api::{message::Pong, RequestData};
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
/// let bind_addr = "127.0.0.1:1234";
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

impl Handler<RequestData> for Turi {
    fn handle(&self, _addr: &SocketAddr, req: RequestData) -> Result<serde_json::Value, BoxError> {
        // handle the actual request
        let res = match req {
            RequestData::Ping(params) => params.handle(|_| Pong),
        };
        Ok(res?)
    }
}

trait TuriRequest: Request<RequestData> + Sized {
    fn handle(
        self,
        handler: impl FnOnce(Self) -> Self::Response,
    ) -> Result<serde_json::Value, BoxError> {
        let res = handler(self);
        Ok(serde_json::to_value(&res)?)
    }
}

impl<T> TuriRequest for T where T: Request<RequestData> {}
