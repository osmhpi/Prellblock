//! A server for communicating between RPUs.

use std::{
    convert::TryInto,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
};

use super::{Calculator, Pong, Request, RequestData};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

type ArcMut<T> = Arc<Mutex<T>>;

/// A receiver (server) instance.
#[derive(Clone)]
pub struct Receiver {
    calculator: ArcMut<Calculator>,
}

impl Receiver {
    /// Create a new receiver instance.
    #[must_use]
    pub fn new(calculator: ArcMut<Calculator>) -> Self {
        Self { calculator }
    }

    /// The main server loop.
    pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
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
                let peer_addr = stream.peer_addr().unwrap();
                log::info!("Connected: {}", peer_addr);
                match clone_self.handle_client(stream) {
                    Ok(()) => log::info!("Disconnected"),
                    Err(err) => log::warn!("Server error: {:?}", err),
                }
            });
        }
        Ok(())
    }

    fn handle_client(self, mut stream: TcpStream) -> Result<(), BoxError> {
        let addr = stream.peer_addr().expect("Peer address");
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
        }
        Ok(())
    }

    fn handle_request(&self, addr: &SocketAddr, req: &[u8]) -> Result<serde_json::Value, BoxError> {
        // TODO: Remove this.
        let _ = self;
        // Deserialize request.
        let req: RequestData = serde_json::from_slice(req)?;
        log::trace!("Received request from {}: {:?}", addr, req);
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
        log::trace!("Send response to {}: {:?}", addr, res);
        Ok(res?)
    }
}

trait ReceiverRequest: Request + Sized {
    fn handle(
        self,
        handler: impl FnOnce(Self) -> Self::Response,
    ) -> Result<serde_json::Value, BoxError> {
        let res = handler(self);
        Ok(serde_json::to_value(&res)?)
    }
}

impl<T> ReceiverRequest for T where T: Request {}
