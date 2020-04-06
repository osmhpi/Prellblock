//! A server for communicating between RPUs.

use std::{
    net::{SocketAddr, TcpListener},
    sync::{Arc, Mutex},
};

use super::{Calculator, Pong, RequestData};
use balise::{
    server::{Handler, Server},
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
/// let bind_addr = "127.0.0.1:1234";
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

impl Handler<RequestData> for Receiver {
    fn handle(&self, _addr: &SocketAddr, req: RequestData) -> Result<serde_json::Value, BoxError> {
        // handle the actual request
        let res = match req {
            RequestData::Add(params) => {
                params.handle(|params| self.calculator.lock().unwrap().add(params.0, params.1))
            }
            RequestData::Sub(params) => {
                params.handle(|params| self.calculator.lock().unwrap().sub(params.0, params.1))
            }
            RequestData::Ping(params) => params.handle(|_| Pong),
        };
        log::debug!(
            "The calculator's last resort is: {}.",
            self.calculator.lock().unwrap().last_result()
        );
        Ok(res?)
    }
}

trait ReceiverRequest: Request<RequestData> + Sized {
    fn handle(
        self,
        handler: impl FnOnce(Self) -> Self::Response,
    ) -> Result<serde_json::Value, BoxError> {
        let res = handler(self);
        Ok(serde_json::to_value(&res)?)
    }
}

impl<T> ReceiverRequest for T where T: Request<RequestData> {}
