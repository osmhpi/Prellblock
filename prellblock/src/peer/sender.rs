//! A client for communicating between RPUs.

use super::{Request, RequestData};
use std::{
    convert::TryInto,
    io::{self, Read, Write},
    net::SocketAddr,
    ops::DerefMut,
    time::{Duration, Instant},
};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

mod connection_pool {
    use super::BoxError;
    use lazy_static::lazy_static;
    use std::{
        collections::HashMap,
        net::{SocketAddr, TcpStream},
        ops::{Deref, DerefMut},
        sync::Mutex,
        time::Duration,
    };

    pub struct ConnectionPool {
        streams: Mutex<HashMap<SocketAddr, Vec<TcpStream>>>,
        timeout: Duration,
    }

    impl ConnectionPool {
        const MAX_STREAMS: usize = 8;

        fn new() -> Self {
            Self {
                streams: HashMap::new().into(),
                timeout: Duration::from_secs(30),
            }
        }

        pub fn stream(&self, addr: SocketAddr) -> Result<StreamGuard, BoxError> {
            let mut streams = self.streams.lock().unwrap();
            let stream = match streams.get_mut(&addr) {
                Some(streams) => match streams.pop() {
                    None => self.connect(&addr),
                    Some(stream) => {
                        match stream.take_error() {
                            Ok(None) => Ok(stream),
                            // arbitrary error with the socket
                            // or an error while retrieving the error
                            _ => self.connect(&addr),
                        }
                    }
                },
                None => self.connect(&addr),
            }?;
            Ok(StreamGuard {
                stream,
                addr,
                pool: self,
            })
        }

        fn add_stream(&self, addr: SocketAddr, stream: TcpStream) {
            let mut streams = self.streams.lock().unwrap();
            match streams.get_mut(&addr) {
                None => {
                    streams.insert(addr, vec![stream]);
                }
                Some(stream_vec) => {
                    if stream_vec.len() < Self::MAX_STREAMS {
                        stream_vec.push(stream)
                    }
                }
            }
        }

        fn connect(&self, addr: &SocketAddr) -> Result<TcpStream, BoxError> {
            let mut seconds = Duration::from_secs(0);
            let delay = Duration::from_secs(1);
            loop {
                let stream = TcpStream::connect(addr);
                if stream.is_ok() || seconds >= self.timeout {
                    break Ok(stream?);
                }
                log::warn!(
                    "Couldn't connect to server at {}, retrying in {:?}.",
                    addr,
                    delay
                );
                std::thread::sleep(delay);
                seconds += delay;
            }
        }
    }

    pub struct StreamGuard<'a> {
        stream: TcpStream,
        addr: SocketAddr,
        pool: &'a ConnectionPool,
    }

    impl<'a> StreamGuard<'a> {
        pub fn done(self) {
            log::trace!("Putting stream into connection pool.");
            self.pool.add_stream(self.addr, self.stream)
        }
    }

    /// This is needed for accessing `TcpStream`'s methods on `StreamGuard`.
    impl<'a> Deref for StreamGuard<'a> {
        type Target = TcpStream;
        fn deref(&self) -> &TcpStream {
            &self.stream
        }
    }

    impl<'a> DerefMut for StreamGuard<'a> {
        fn deref_mut(&mut self) -> &mut TcpStream {
            &mut self.stream
        }
    }

    lazy_static! {
        pub static ref POOL: ConnectionPool = ConnectionPool::new();
    }
}

/// A sender instance.
///
/// The sender keeps up a connection pool of open connections
/// for improved efficiency.
pub struct Sender {
    addr: SocketAddr,
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
        Self { addr }
    }

    /// Send a request to the server specified.
    pub fn send_request<Req>(&mut self, req: Req) -> Result<Req::Response, BoxError>
    where
        Req: Request,
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

fn send_request<S, Req>(stream: &mut S, req: Req) -> Result<Result<Req::Response, String>, BoxError>
where
    S: Read + Write,
    Req: Request,
{
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

    let res = serde_json::from_slice(&buf)?;
    Ok(res)
}

struct Stream<R, W>(R, W);

impl<R, W> Read for Stream<R, W>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.0.read(buf)
    }
}

impl<R, W> Write for Stream<R, W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.1.write(buf)
    }
    fn flush(&mut self) -> Result<(), io::Error> {
        self.1.flush()
    }
}

#[test]
fn test_send_request_encoding() {
    use super::message;
    use std::io::Cursor;
    let mut write_buf = Vec::new();
    let writer = Cursor::new(&mut write_buf);
    let reader = Cursor::new(b"\x0b\0\0\0{\"Ok\":null}");
    let mut stream = Stream(reader, writer);
    let request = message::Ping;
    send_request(&mut stream, request).unwrap().unwrap();
    assert_eq!(write_buf, b"\x0d\0\0\0{\"Ping\":null}");
}
