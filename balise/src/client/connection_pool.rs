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
    sync::{Condvar, Mutex},
};
use stream_impl::StreamImpl;

pub struct ConnectionPool {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    streams: HashMap<SocketAddr, Vec<StreamImpl>>,
    current_streams: usize,
}

impl ConnectionPool {
    const MAX_STREAMS: usize = 512;

    fn new() -> Self {
        Self {
            state: Mutex::default(),
            changed: Condvar::default(),
        }
    }

    pub fn stream(&self, addr: SocketAddr) -> Result<StreamGuard, BoxError> {
        let mut state = self
            .changed
            .wait_while(self.state.lock().unwrap(), |state| {
                state.current_streams >= Self::MAX_STREAMS
            })
            .unwrap();
        state.current_streams += 1;
        let stream = state.streams.get_mut(&addr).and_then(Vec::pop);
        drop(state);

        let stream = match stream {
            Some(stream) => stream,
            None => stream_impl::connect(&addr)?,
        };
        Ok(StreamGuard {
            stream: Some(stream),
            addr,
            pool: self,
        })
    }

    fn add_stream(&self, addr: SocketAddr, stream: StreamImpl) {
        let mut state = self.state.lock().unwrap();
        match state.streams.get_mut(&addr) {
            None => {
                state.streams.insert(addr, vec![stream]);
            }
            Some(stream_vec) => {
                if stream_vec.len() < Self::MAX_STREAMS {
                    stream_vec.push(stream)
                }
            }
        }
        state.current_streams -= 1;
        self.changed.notify_all();
    }

    fn lost_stream(&self) {
        let mut state = self.state.lock().unwrap();
        state.current_streams -= 1;
        self.changed.notify_all();
    }
}

pub struct StreamGuard<'a> {
    stream: Option<StreamImpl>,
    addr: SocketAddr,
    pool: &'a ConnectionPool,
}

impl<'a> StreamGuard<'a> {
    pub fn done(mut self) {
        log::trace!("Putting stream into connection pool.");
        if let Some(stream) = self.stream.take() {
            self.pool.add_stream(self.addr, stream);
        }
    }
}

impl<'a> Drop for StreamGuard<'a> {
    fn drop(&mut self) {
        if self.stream.is_some() {
            self.pool.lost_stream();
        }
    }
}

/// This is needed for accessing `StreamImpl`'s methods on `StreamGuard`.
impl<'a> Deref for StreamGuard<'a> {
    type Target = StreamImpl;
    fn deref(&self) -> &StreamImpl {
        self.stream.as_ref().unwrap()
    }
}

impl<'a> DerefMut for StreamGuard<'a> {
    fn deref_mut(&mut self) -> &mut StreamImpl {
        self.stream.as_mut().unwrap()
    }
}

lazy_static! {
    pub static ref POOL: ConnectionPool = ConnectionPool::new();
}
