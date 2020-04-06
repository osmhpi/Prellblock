use super::BoxError;
use std::net::{SocketAddr, TcpStream};

pub type StreamImpl = TcpStream;

impl<'a> super::StreamGuard<'a> {
    pub const fn tcp_stream(&self) -> &TcpStream {
        &self.stream
    }
}

pub fn connect(addr: &SocketAddr) -> Result<StreamImpl, BoxError> {
    let stream = TcpStream::connect(addr)?;
    Ok(stream)
}
