//! A server for communicating between RPUs.

use std::{
    net::{SocketAddr, TcpListener},
    sync::{Arc, Mutex},
};

use super::{message, Calculator, PeerMessage, Pong};
use crate::datastorage::DataStorage;
use balise::{
    server::{Handler, Response, Server},
    Request,
};
use pinxit::Signable;
use prellblock_client_api::TransactionMessage;

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
    tls_identity: String,
    calculator: ArcMut<Calculator>,
    data_storage: Arc<DataStorage>,
}

impl Receiver {
    /// Create a new receiver instance.
    #[must_use]
    pub fn new(
        tls_identity: String,
        calculator: ArcMut<Calculator>,
        data_storage: Arc<DataStorage>,
    ) -> Self {
        Self {
            tls_identity,
            calculator,
            data_storage,
        }
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
        let tls_identity = self.tls_identity.clone();
        let server = Server::new(self, tls_identity, "prellblock")?;
        server.serve(listener)
    }

    fn handle_set_value(&self, params: message::SetValue) -> Result<(), BoxError> {
        let message::SetValue(peer_id, key, value, signature) = params;
        // Check validity of message signature.
        TransactionMessage {
            key: &key,
            value: &value,
        }
        .verify(&peer_id, &signature)?;
        log::info!(
            "Client {} set {} to {} (via another RPU)",
            peer_id,
            key,
            value
        );

        // TODO: Continue with warning or error?
        self.data_storage.write(&peer_id, key, &value)?;

        Ok(())
    }
}

impl Handler<PeerMessage> for Receiver {
    fn handle(&self, _addr: &SocketAddr, req: PeerMessage) -> Result<Response, BoxError> {
        // handle the actual request
        let res = match req {
            PeerMessage::Add(params) => {
                params.handle(|params| Ok(self.calculator.lock().unwrap().add(params.0, params.1)))
            }
            PeerMessage::Sub(params) => {
                params.handle(|params| Ok(self.calculator.lock().unwrap().sub(params.0, params.1)))
            }
            PeerMessage::Ping(params) => params.handle(|_| Ok(Pong)),
            PeerMessage::SetValue(params) => params.handle(|params| self.handle_set_value(params)),
        };
        Ok(res?)
    }
}
