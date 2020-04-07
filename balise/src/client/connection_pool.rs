#[cfg(feature = "tls")]
#[path = "stream_impl_tls.rs"]
mod stream_impl;

#[cfg(not(feature = "tls"))]
#[path = "stream_impl_tcp.rs"]
mod stream_impl;

use super::BoxError;
use lazy_static::lazy_static;
use std::{
    collections::HashMap,
    net::SocketAddr,
    ops::{Deref, DerefMut},
    sync::Mutex,
};
use stream_impl::StreamImpl;

pub struct ConnectionPool {
    streams: Mutex<HashMap<SocketAddr, Vec<StreamImpl>>>,
}

impl ConnectionPool {
    const MAX_STREAMS: usize = 8;

    fn new() -> Self {
        Self {
            streams: HashMap::new().into(),
        }
    }

    pub fn stream(&self, addr: SocketAddr) -> Result<StreamGuard, BoxError> {
        let mut streams = self.streams.lock().unwrap();
        let stream = streams.get_mut(&addr).and_then(Vec::pop);
        drop(streams);

        let stream = match stream {
            Some(stream) => stream,
            None => stream_impl::connect(&addr)?,
        };
        Ok(StreamGuard {
            stream,
            addr,
            pool: self,
        })
    }

    fn add_stream(&self, addr: SocketAddr, stream: StreamImpl) {
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
}

pub struct StreamGuard<'a> {
    stream: StreamImpl,
    addr: SocketAddr,
    pool: &'a ConnectionPool,
}

impl<'a> StreamGuard<'a> {
    pub fn done(self) {
        log::trace!("Putting stream into connection pool.");
        self.pool.add_stream(self.addr, self.stream)
    }
}

/// This is needed for accessing `StreamImpl`'s methods on `StreamGuard`.
impl<'a> Deref for StreamGuard<'a> {
    type Target = StreamImpl;
    fn deref(&self) -> &StreamImpl {
        &self.stream
    }
}

impl<'a> DerefMut for StreamGuard<'a> {
    fn deref_mut(&mut self) -> &mut StreamImpl {
        &mut self.stream
    }
}

lazy_static! {
    pub static ref POOL: ConnectionPool = ConnectionPool::new();
}
