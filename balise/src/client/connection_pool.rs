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
