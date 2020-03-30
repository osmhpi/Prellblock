//! A client for communicating between RPUs.

use super::{Request, RequestData};
use std::{
    convert::TryInto,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A sender instance.
pub struct Sender {
    addr: SocketAddr,
    stream: Option<TcpStream>,
    timeout: Duration,
}

impl Sender {
    /// Create a new sender instance.
    ///
    /// # Example
    ///
    /// ```
    /// use prellblock::peer::Sender;
    ///
    /// let addr = "127.0.0.1:2480".parse().unwrap();
    /// let sender = Sender::new(addr);
    /// ```
    #[must_use]
    pub const fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            stream: None,
            timeout: Duration::from_secs(60),
        }
    }

    fn get_stream(&mut self) -> Result<&TcpStream, BoxError> {
        if self.stream.is_none() {
            self.stream = Some(self.connect()?);
        }
        Ok(self.stream.as_ref().unwrap())
    }

    fn connect(&self) -> Result<TcpStream, BoxError> {
        let mut seconds = Duration::from_secs(0);
        let delay = Duration::from_secs(1);
        loop {
            let stream = TcpStream::connect(self.addr);
            if stream.is_ok() || seconds >= self.timeout {
                break Ok(stream?);
            }
            log::warn!(
                "Couldn't connect to server at {}, retrying in {:?}.",
                self.addr,
                delay
            );
            std::thread::sleep(delay);
            seconds += delay;
        }
    }

    /// Send a request to the server specified.
    pub fn send_request<Req>(&mut self, req: Req) -> Result<Req::Response, BoxError>
    where
        Req: Request,
    {
        let mut stream = self.get_stream()?;
        let addr = stream.peer_addr()?;

        log::trace!("Sending request to {}: {:?}", addr, req);

        let req: RequestData = req.into();

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
        let res: Result<Req::Response, String> = serde_json::from_slice(&buf)?;
        log::trace!("Received response from {}: {:?}", addr, res);
        Ok(res?)
    }
}
