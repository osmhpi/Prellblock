//! A client for communicating between RPUs.

mod connection_pool;

use super::{BoxError, Request};
use serde::Serialize;
use std::{
    convert::TryInto,
    io::{self, Read, Write},
    marker::PhantomData,
    net::SocketAddr,
    ops::DerefMut,
    time::{Duration, Instant},
};

/// A client instance.
///
/// The client keeps up a connection pool of open connections
/// for improved efficiency.
pub struct Client<T> {
    addr: SocketAddr,
    request_data: PhantomData<T>,
}

impl<T> Client<T> {
    /// Create a new client instance.
    ///
    /// # Example
    ///
    /// ```
    /// use balise::client::Client;
    ///
    /// let addr = "127.0.0.1:2480".parse().unwrap();
    /// let client = Client::<()>::new(addr);
    /// ```
    #[must_use]
    pub const fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            request_data: PhantomData,
        }
    }

    /// Send a request to the server specified.
    pub fn send_request<Req>(&mut self, req: Req) -> Result<Req::Response, BoxError>
    where
        Req: Request<T>,
        T: Serialize,
    {
        let (mut stream, addr) = self.stream()?;

        log::trace!("Sending request to {}: {:?}", addr, req);
        let res = send_request(stream.deref_mut(), req)?;

        log::trace!("Received response from {}: {:?}", addr, res);
        stream.done();
        Ok(res?)
    }

    /// Get a working TCP stream.
    ///
    /// A stream could be closed by the receiver while being
    /// in the pool. This is catched and a new stream will be
    /// returned in this case.
    fn stream(&self) -> Result<(connection_pool::StreamGuard, SocketAddr), BoxError> {
        let deadline = Instant::now() + Duration::from_secs(60);

        let res = loop {
            let stream = connection_pool::POOL.stream(self.addr)?;
            let addr = stream.peer_addr()?;

            if Instant::now() > deadline {
                return Err("Timeout: Could not send request.".into());
            }

            // check TCP connection functional
            stream.set_nonblocking(true)?;

            //read one byte without removing from message queue
            let mut buf = [0; 1];
            match stream.peek(&mut buf) {
                Ok(n) => {
                    if n > 0 {
                        log::warn!("The Receiver is not working correctly!");
                    }
                    // no connection
                    let local_addr = stream.local_addr().unwrap();
                    log::trace!(
                        "TCP connection from {} to {} seems to be broken.",
                        local_addr,
                        addr
                    );
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // blocking means stream is ok
                    stream.set_nonblocking(false)?;
                    break (stream, addr);
                }
                Err(e) => return Err(e.into()),
            }
        };
        Ok(res)
    }
}

fn send_request<S, Req, T>(
    stream: &mut S,
    req: Req,
) -> Result<Result<Req::Response, String>, BoxError>
where
    S: Read + Write,
    Req: Request<T>,
    T: Serialize,
{
    let req: T = req.into();
    // serialize request
    let data = serde_json::to_vec(&req)?;
    // send request
    let size: u32 = data.len().try_into()?;
    let size = size.to_le_bytes();
    stream.write_all(&size)?;
    stream.write_all(&data)?;
    // read response length
    let mut len_buf = [0; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    // read message
    let mut buf = vec![0; len];
    stream.read_exact(&mut buf)?;

    let res = serde_json::from_slice(&buf)?;
    Ok(res)
}
