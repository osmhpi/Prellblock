//! A server for communicating between RPUs.

use std::{
    net::TcpListener,
    sync::{Arc, Mutex},
};

use super::{message, Calculator, PeerMessage, Pong};
use crate::data_storage::DataStorage;
use balise::{
    handle_fn,
    server::{Handler, Server},
    Request,
};
use prellblock_client_api::Transaction;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

type ArcMut<T> = Arc<Mutex<T>>;

/// A receiver (server) instance.
///
/// # Example
///
/// ```
/// use prellblock::{
///     data_storage::DataStorage,
///     peer::{Calculator, Receiver},
/// };
/// use std::{net::TcpListener, sync::Arc};
///
/// let calculator = Calculator::new();
/// let calculator = Arc::new(calculator.into());
/// let bind_addr = "127.0.0.1:0"; // replace 0 with a real port
///
/// let data_storage = DataStorage::new("/tmp/some_db").unwrap();
///
/// let listener = TcpListener::bind(bind_addr).unwrap();
/// let receiver = Receiver::new(
///     "path_to_pfx.pfx".to_string(),
///     calculator,
///     Arc::new(data_storage),
/// );
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

    fn handle_execute(&self, params: message::Execute) -> Result<(), BoxError> {
        let message::Execute(peer_id, transaction) = params;
        let transaction = transaction.verify(&peer_id)?;

        match transaction.into_inner() {
            Transaction::KeyValue { key, value } => {
                log::info!(
                    "Client {} set {} to {} (via another RPU)",
                    &peer_id,
                    key,
                    value
                );

                // TODO: Continue with warning or error?
                self.data_storage.write(&peer_id, key, &value)?;
            }
        }

        Ok(())
    }
}

impl Handler<PeerMessage> for Receiver {
    handle_fn!(self, PeerMessage, {
        Add(params) =>  Ok(self.calculator.lock().unwrap().add(params.0, params.1)),
        Sub(params) =>  Ok(self.calculator.lock().unwrap().sub(params.0, params.1)),
        Ping(_) => Ok(Pong),
        Execute(params) => self.handle_execute(params),
    });
}
