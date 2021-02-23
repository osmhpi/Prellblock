//! A client for communicating between RPUs.

mod connection_pool;

use crate::{Address, Error, Request};
use serde::Serialize;
use std::{
    convert::TryInto,
    marker::{PhantomData, Unpin},
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// A client instance.
///
/// The client keeps up a connection pool of open connections
/// for improved efficiency.
pub struct Client<T> {
    addr: Address,
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
    pub const fn new(addr: Address) -> Self {
        Self {
            addr,
            request_data: PhantomData,
        }
    }

    /// Send a request to the server specified.
    pub async fn send_request<Req>(&mut self, req: Req) -> Result<Req::Response, Error>
    where
        Req: Request<T>,
        T: Serialize,
    {
        let (mut stream, addr) = self.stream().await?;

        log::trace!("Sending request to {}: {:?}", addr, req);
        let res = send_request(&mut *stream, req).await?;

        log::trace!("Received response from {}: {:?}", addr, res);
        stream.done().await;
        Ok(res?)
    }

    /// Get a working TCP stream.
    ///
    /// A stream could be closed by the receiver while being
    /// in the pool. This is catched and a new stream will be
    /// returned in this case.
    async fn stream(&self) -> Result<(connection_pool::StreamGuard<'_>, SocketAddr), Error> {
        let deadline = Instant::now() + Duration::from_secs(3);
        let delay = Duration::from_secs(1);

        let res = loop {
            if Instant::now() > deadline {
                return Err(Error::Timeout);
            }

            let stream = match connection_pool::POOL.stream(self.addr.clone()).await {
                Ok(stream) => stream,
                Err(err) => {
                    log::warn!(
                        "Couldn't connect to server at {}, retrying in {:?}: {}",
                        self.addr,
                        delay,
                        err
                    );
                    std::thread::sleep(delay);
                    continue;
                }
            };
            let addr = stream.tcp_stream().peer_addr()?;

            // // check TCP connection functional
            // stream.tcp_stream().set_nonblocking(true)?;

            // //read one byte without removing from message queue
            // let mut buf = [0; 1];
            // match stream.tcp_stream().peek(&mut buf) {
            //     Ok(n) => {
            //         if n > 0 {
            //             log::warn!("The Receiver is not working correctly!");
            //         }
            //         // no connection
            //         let local_addr = stream.tcp_stream().local_addr().unwrap();
            //         log::trace!(
            //             "TCP connection from {} to {} seems to be broken.",
            //             local_addr,
            //             addr
            //         );
            //     }
            //     Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
            //         // blocking means stream is ok
            //         stream.tcp_stream().set_nonblocking(false)?;
            //         break (stream, addr);
            //     }
            //     Err(e) => return Err(e.into()),
            // }
            break (stream, addr);
        };
        Ok(res)
    }
}

async fn send_request<S, Req, T>(
    stream: &mut S,
    req: Req,
) -> Result<Result<Req::Response, String>, Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
    Req: Request<T>,
    T: Serialize,
{
    let req: T = req.into();
    // serialize request
    let vec = vec![0; 4];
    let mut vec = postcard::serialize_with_flavor(&req, postcard::flavors::StdVec(vec))?;
    // send request
    let size: u32 = (vec.len() - 4)
        .try_into()
        .map_err(|_| Error::MessageTooLong)?;
    vec[..4].copy_from_slice(&size.to_le_bytes());
    stream.write_all(&vec).await?;
    // read response length
    let mut len_buf = [0; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    // read message
    let mut buf = vec![0; len];
    stream.read_exact(&mut buf).await?;

    let res = match postcard::from_bytes(&buf)? {
        Ok(data) => Ok(postcard::from_bytes(data)?),
        Err(err) => Err(err),
    };
    Ok(res)
}
