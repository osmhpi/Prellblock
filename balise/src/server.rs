//! A server for communicating between RPUs.

use super::BoxError;
use serde::de::DeserializeOwned;
use std::{
    convert::TryInto,
    fmt::Debug,
    io::{self, Read, Write},
    marker::PhantomData,
    net::{SocketAddr, TcpListener},
    path::Path,
    sync::Arc,
};

/// A transparent response to a `Request`.
///
/// Use the `handle` method to create a matching response.
pub struct Response(pub(crate) serde_json::Value);

#[cfg(feature = "tls")]
use native_tls::{Identity, Protocol, TlsAcceptor};

#[cfg(feature = "tls")]
pub use native_tls::Identity as TlsIdentity;

#[cfg(not(feature = "tls"))]
struct TlsAcceptor;

#[cfg(not(feature = "tls"))]
impl TlsAcceptor {
    fn accept<T>(&self, stream: T) -> Result<T, String> {
        Ok(stream)
    }
}

/// A Server (server) instance.
pub struct Server<T, H> {
    request_data: PhantomData<T>,
    handler: H,
    acceptor: Arc<TlsAcceptor>,
}

impl<T, H> Clone for Server<T, H>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            request_data: PhantomData,
            handler: self.handler.clone(),
            acceptor: self.acceptor.clone(),
        }
    }
}

impl<T, H> Server<T, H>
where
    T: DeserializeOwned + Debug,
    H: Handler<T> + Clone,
{
    /// Create a new server instance.
    ///
    /// The `handler` needs to provide a `handle` callback script to handle requests on the server.
    #[must_use]
    #[cfg(not(feature = "tls"))]
    pub fn new(handler: H) -> Self {
        Self {
            request_data: PhantomData,
            handler,
            acceptor: Arc::new(TlsAcceptor),
        }
    }

    /// Create a new TLS server instance.
    ///
    /// The `handler` needs to provide a `handle` callback script to handle requests on the server.
    /// The `identity` determines the server's identity.
    #[cfg(feature = "tls")]
    pub fn new(handler: H, identity: Identity) -> Result<Self, BoxError> {
        let acceptor = TlsAcceptor::builder(identity)
            .min_protocol_version(Some(Protocol::Tlsv12))
            .build()?;
        let acceptor = Arc::new(acceptor);

        Ok(Self {
            request_data: PhantomData,
            handler,
            acceptor,
        })
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError>
    where
        T: Send + 'static,
        H: Send + 'static,
    {
        log::info!(
            "Server is now listening on Port {}",
            listener.local_addr()?.port()
        );
        for stream in listener.incoming() {
            // TODO: Is there a case where we should continue to listen for incoming streams?
            let stream = stream?;

            let clone_self = self.clone();

            // handle the client in a new thread
            std::thread::spawn(move || {
                let peer_addr = stream.peer_addr().expect("Peer address");
                log::info!("Connected: {}", peer_addr);

                let result = clone_self
                    .acceptor
                    .accept(stream)
                    .map_err(Into::into) //rust is geil.
                    .and_then(|stream| clone_self.handle_client(peer_addr, stream));
                match result {
                    Ok(()) => log::info!("Disconnected"),
                    Err(err) => log::warn!("Server error: {:?}", err),
                }
            });
        }
        Ok(())
    }

    fn handle_client<S>(self, addr: SocketAddr, mut stream: S) -> Result<(), BoxError>
    where
        S: Read + Write,
    {
        loop {
            // read message length
            let mut len_buf = [0; 4];
            match stream.read_exact(&mut len_buf) {
                Ok(()) => {}
                Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err.into()),
            };

            let len = u32::from_le_bytes(len_buf) as usize;

            // read message
            let mut buf = vec![0; len];
            stream.read_exact(&mut buf)?;

            // handle the request
            let res = match self.handle_request(&addr, &buf) {
                Ok(res) => Ok(res),
                Err(err) => Err(err.to_string()),
            };

            // serialize response
            let mut vec = vec![0; 4];
            serde_json::to_writer(&mut vec, &res)?;

            // send response
            let size: u32 = (vec.len() - 4).try_into()?;
            vec[..4].copy_from_slice(&size.to_le_bytes());
            stream.write_all(&vec)?;

            // Simulate connection drop
            // let _ = stream.shutdown(std::net::Shutdown::Both);
            // break;
        }
        Ok(())
    }

    fn handle_request(&self, addr: &SocketAddr, req: &[u8]) -> Result<serde_json::Value, BoxError> {
        // TODO: Remove this.
        let _ = self;
        // Deserialize request.
        let req: T = serde_json::from_slice(req)?;
        log::trace!("Received request from {}: {:?}", addr, req);
        // handle the actual request
        let res = self.handler.handle(addr, req).map(|response| response.0);
        log::trace!("Send response to {}: {:?}", addr, res);
        Ok(res?)
    }
}

/// Load the identity from a file path.
///
/// `identity_path` is a file path to a `.pfx` file containing the server's identity.
/// This file could be protected by a `password`.
#[cfg(feature = "tls")]
pub fn load_identity(
    identity_path: impl AsRef<Path>,
    password: &str,
) -> Result<Identity, io::Error> {
    log::trace!(
        "Loading server identity from {}.",
        identity_path.as_ref().display()
    );
    let identity = std::fs::read(identity_path)?;
    Identity::from_pkcs12(&identity, password)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

/// Handles a request and returns the corresponding response.
pub trait Handler<T> {
    /// Handle the request.
    fn handle(&self, addr: &SocketAddr, req: T) -> Result<Response, BoxError>;
}
