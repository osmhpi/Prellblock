//! A server for communicating between RPUs.

use super::Request;
use serde::de::DeserializeOwned;
use std::{
    convert::TryInto,
    fmt::Debug,
    io::{self, Read, Write},
    marker::PhantomData,
    net::{SocketAddr, TcpListener},
    sync::Arc,
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[cfg(feature = "tls")]
use native_tls::{Identity, Protocol, TlsAcceptor};

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
    /// The `identity_path` is a file path to a `.pfx` file containing the server's identity.
    #[must_use]
    #[cfg(feature = "tls")]
    pub fn new(handler: H, identity_path: String, password: String) -> Result<Self, BoxError> {
        log::trace!("Load server identity from {}.", identity_path);
        let identity = std::fs::read(identity_path)?;
        let identity = Identity::from_pkcs12(&identity, &password)?;

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
            let data = serde_json::to_vec(&res)?;

            // send response
            let size: u32 = data.len().try_into()?;
            let size = size.to_le_bytes();
            stream.write_all(&size)?;
            stream.write_all(&data)?;

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
        let res = self.handler.handle(addr, req);
        log::trace!("Send response to {}: {:?}", addr, res);
        Ok(res?)
    }
}

/// Handles a request and returns the corresponding response.
pub trait Handler<T> {
    /// Handle the request.
    fn handle(&self, addr: &SocketAddr, req: T) -> Result<serde_json::Value, BoxError>;
}

trait ServerRequest<T>: Request<T> + Sized {
    fn handle(
        self,
        handler: impl FnOnce(Self) -> Self::Response,
    ) -> Result<serde_json::Value, BoxError> {
        let res = handler(self);
        Ok(serde_json::to_value(&res)?)
    }
}

// fn serve() {
//     let identiy = File::Read("identity.pfx")?;
//     let identity = Identity::from_pkcs12(&identity, "").unwrap();

//     let listener = TcpListener::bind("127.0.0.1:8443").unwrap();
//     let acceptor = TlsAcceptor::new(identity).unwrap();
//     let acceptor = Arc::new(acceptor);

//     fn handle_client(stream: TlsStream<TcpStream>) {
//         log::info!("Connected to Client");
//         // ...
//         thread::sleep(Duration::new(2, 0));
//     }
//     log::info!("Starting to listen for incoming Streams");
//     for stream in listener.incoming() {
//         match stream {
//             Ok(stream) => {
//                 let acceptor = acceptor.clone();
//                 thread::spawn(move || {
//                     let stream = acceptor.accept(stream).unwrap();
//                     handle_client(stream);
//                 });
//             }
//             Err(_e) => { /* connection failed */ }
//         }
//     }
// }

impl<R, T> ServerRequest<T> for R where R: Request<T> {}
