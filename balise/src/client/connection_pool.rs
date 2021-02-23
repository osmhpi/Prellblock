#[cfg(feature = "tls")]
#[path = "stream_impl_tls.rs"]
mod stream_impl;

#[cfg(not(feature = "tls"))]
#[path = "stream_impl_tcp.rs"]
mod stream_impl;

use crate::{Address, Error};
use lazy_static::lazy_static;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};
use stream_impl::StreamImpl;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

pub struct ConnectionPool {
    states: Mutex<HashMap<Address, State>>,
}

struct State {
    streams: Vec<StreamImpl>,
    current_streams: Arc<Semaphore>,
}

impl ConnectionPool {
    const MAX_STREAMS: usize = 64;

    fn new() -> Self {
        Self {
            states: Mutex::default(),
        }
    }

    pub async fn stream(&self, addr: Address) -> Result<StreamGuard<'_>, Error> {
        let mut states = self.states.lock().await;
        let (current_streams, stream) = if let Some(state) = states.get_mut(&addr) {
            (state.current_streams.clone(), state.streams.pop())
        } else {
            let current_streams = Arc::new(Semaphore::new(Self::MAX_STREAMS));
            states.insert(
                addr.clone(),
                State {
                    streams: Vec::new(),
                    current_streams: current_streams.clone(),
                },
            );
            (current_streams, None)
        };
        drop(states);
        let permit = current_streams.acquire_owned().await;
        let permit = permit.expect("unable to acquire");

        let stream = match stream {
            Some(stream) => stream,
            None => stream_impl::connect(&addr).await?,
        };
        Ok(StreamGuard {
            stream: Some(stream),
            addr,
            pool: self,
            permit,
        })
    }

    /// Add an existing `stream` back into the pool for the given `addr`.
    async fn add_stream(&self, addr: Address, stream: StreamImpl) {
        let mut states = self.states.lock().await;
        let state = states.get_mut(&addr).unwrap();
        state.streams.push(stream);
    }
}

pub struct StreamGuard<'a> {
    stream: Option<StreamImpl>,
    addr: Address,
    pool: &'a ConnectionPool,
    /// This has to be stored in the guard.
    /// On drop, this will signal the semaphore (number of connections).
    #[allow(dead_code)]
    permit: OwnedSemaphorePermit,
}

impl<'a> StreamGuard<'a> {
    pub async fn done(mut self) {
        log::trace!("Putting stream into connection pool.");
        if let Some(stream) = self.stream.take() {
            self.pool.add_stream(self.addr, stream).await;
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
