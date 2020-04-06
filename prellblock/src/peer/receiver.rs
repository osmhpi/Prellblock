//! A server for communicating between RPUs.

use std::{
    net::{SocketAddr, TcpListener},
    sync::{Arc, Mutex},
};

use super::{Calculator, PeerMessage, Pong};
use balise::{
    server::{Handler, Response, Server},
    Request,
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

type ArcMut<T> = Arc<Mutex<T>>;

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
/// let bind_addr = "127.0.0.1:0"; // replace 0 with a real port
///
/// let listener = TcpListener::bind(bind_addr).unwrap();
/// let receiver = Receiver::new(calculator, "path_to_pfx.pfx".to_string());
/// std::thread::spawn(move || {
///     receiver.serve(&listener).unwrap();
/// });
/// ```
#[derive(Clone)]
pub struct Receiver {
    calculator: ArcMut<Calculator>,
    tls_identity: String,
}

impl Receiver {
    /// Create a new receiver instance.
    #[must_use]
    pub fn new(calculator: ArcMut<Calculator>, tls_identity: String) -> Self {
        Self {
            calculator,
            tls_identity,
        }
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
        let tls_identity = self.tls_identity.clone();
        let server = Server::new(self, tls_identity, "prellblock")?;
        server.serve(listener)
    }
}

impl Handler<PeerMessage> for Receiver {
    fn handle(&self, _addr: &SocketAddr, req: PeerMessage) -> Result<Response, BoxError> {
        // handle the actual request
        let res = match req {
            PeerMessage::Add(params) => {
                params.handle(|params| self.calculator.lock().unwrap().add(params.0, params.1))
            }
            PeerMessage::Sub(params) => {
                params.handle(|params| self.calculator.lock().unwrap().sub(params.0, params.1))
            }
            PeerMessage::Ping(params) => params.handle(|_| Pong),
        };
        log::debug!(
            "The calculator's last resort is: {}.",
            self.calculator.lock().unwrap().last_result()
        );
        Ok(res?)
    }
}
