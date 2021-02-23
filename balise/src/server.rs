//! A server for communicating between RPUs.

use crate::{Error, Request};
use serde::de::DeserializeOwned;
use std::{
    convert::TryInto,
    fmt::Debug,
    future::Future,
    io,
    marker::{PhantomData, Unpin},
    net::SocketAddr,
    sync::Arc,
};
use tokio::{
    fs,
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpListener,
};

type ServerResult = Result<Response, Error>;

/// A transparent response to a `Request`.
///
/// Use the `handle` method to create a matching response.
pub struct Response(pub(crate) Vec<u8>);

#[cfg(feature = "tls")]
pub use native_tls::Identity as TlsIdentity;

#[cfg(feature = "tls")]
use ::{
    native_tls::{Identity, Protocol, TlsAcceptor},
    std::path::Path,
    tokio_native_tls::TlsAcceptor as AsyncTlsAcceptor,
};

#[cfg(not(feature = "tls"))]
struct AsyncTlsAcceptor;

#[cfg(not(feature = "tls"))]
impl AsyncTlsAcceptor {
    async fn accept<T>(&self, stream: T) -> Result<T, String> {
        Ok(stream)
    }
}

/// A Server (server) instance.
pub struct Server<T, H> {
    request_data: PhantomData<fn() -> T>,
    handler: H,
    acceptor: Arc<AsyncTlsAcceptor>,
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

impl<T, H, F> Server<T, H>
where
    T: DeserializeOwned + Debug,
    H: FnOnce(T) -> F + Clone + Sync,
    F: Future<Output = Result<Response, Error>> + Send,
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
            acceptor: Arc::new(AsyncTlsAcceptor),
        }
    }

    /// Create a new TLS server instance.
    ///
    /// The `handler` needs to provide a `handle` callback script to handle requests on the server.
    /// The `identity` determines the server's identity.
    #[cfg(feature = "tls")]
    pub fn new(handler: H, identity: Identity) -> Result<Self, Error> {
        let acceptor = TlsAcceptor::builder(identity)
            .min_protocol_version(Some(Protocol::Tlsv12))
            .build()?;
        let acceptor = Arc::new(acceptor.into());

        Ok(Self {
            request_data: PhantomData,
            handler,
            acceptor,
        })
    }

    /// The main server loop.
    pub async fn serve(self, listener: &mut TcpListener) -> Result<(), Error>
    where
        T: Send + 'static,
        H: Send + 'static,
    {
        log::info!(
            "Server is now listening on Port {}",
            listener.local_addr()?.port()
        );
        loop {
            // TODO: Is there a case where we should continue to listen for incoming streams?
            let (stream, _) = listener.accept().await?;

            let clone_self = self.clone();

            // handle the client in a new thread
            tokio::spawn(async move {
                let peer_addr = stream.peer_addr().expect("Peer address");
                log::info!("Connected: {}", peer_addr);

                let result = match clone_self.acceptor.accept(stream).await {
                    Ok(stream) => clone_self.handle_client(peer_addr, stream).await,
                    Err(err) => Err(err.into()),
                };
                match result {
                    Ok(()) => log::info!("Disconnected"),
                    Err(err) => log::warn!("Server error: {:?}", err),
                }
            });
        }
    }

    async fn handle_client<S>(self, addr: SocketAddr, mut stream: S) -> Result<(), Error>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        loop {
            // read message length
            let mut len_buf = [0; 4];
            match stream.read_exact(&mut len_buf).await {
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(Error::IO(err)),
            };

            let len = u32::from_le_bytes(len_buf) as usize;

            // read message
            let mut buf = vec![0; len];
            stream.read_exact(&mut buf).await?;

            // handle the request
            let res = match self.handle_request(&addr, &buf).await {
                Ok(res) => Ok(res),
                Err(err) => Err(err.to_string()),
            };

            // serialize response
            let vec = vec![0; 4];
            let mut vec = postcard::serialize_with_flavor(&res, postcard::flavors::StdVec(vec))?;

            // send response
            let size: u32 = (vec.len() - 4)
                .try_into()
                .map_err(|_| Error::MessageTooLong)?;
            vec[..4].copy_from_slice(&size.to_le_bytes());
            stream.write_all(&vec).await?;

            // Simulate connection drop
            // let _ = stream.shutdown(std::net::Shutdown::Both);
            // break;
        }
        Ok(())
    }

    async fn handle_request(&self, addr: &SocketAddr, req: &[u8]) -> Result<Vec<u8>, Error> {
        // Deserialize request.
        let req: T = postcard::from_bytes(req)?;
        log::trace!("Received request from {}: {:?}", addr, req);
        // handle the actual request
        let res = (self.handler.clone())(req).await.map(|response| response.0);
        log::trace!("Send response to {}: {:?}", addr, res);
        Ok(res?)
    }
}

/// Load the identity from a file path.
///
/// `identity_path` is a file path to a `.pfx` file containing the server's identity.
/// This file could be protected by a `password`.
#[cfg(feature = "tls")]
pub async fn load_identity(
    identity_path: impl AsRef<Path>,
    password: &str,
) -> Result<Identity, io::Error> {
    log::trace!(
        "Loading server identity from {}.",
        identity_path.as_ref().display()
    );
    let identity = fs::read(identity_path).await?;
    Identity::from_pkcs12(&identity, password)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

/// Call the request `handler` and encode the response.
pub async fn handle_params<T, R, H, F>(params: R, handler: H) -> ServerResult
where
    R: Request<T>,
    H: FnOnce(R) -> F,
    F: Future<Output = Result<R::Response, crate::BoxError>>,
{
    let res = handler(params).await?;
    let data = postcard::to_stdvec(&res)?;
    Ok(Response(data))
}
