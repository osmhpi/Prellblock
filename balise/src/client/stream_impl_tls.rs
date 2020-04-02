use super::BoxError;
use native_tls::{Certificate, TlsConnector, TlsStream};
use std::{
    fs,
    net::{SocketAddr, TcpStream},
};

pub type StreamImpl = TlsStream<TcpStream>;

impl<'a> super::StreamGuard<'a> {
    pub fn tcp_stream(&self) -> &TcpStream {
        self.stream.get_ref()
    }
}

pub fn connect(addr: &SocketAddr) -> Result<StreamImpl, BoxError> {
    // open certificate file
    let buffer = fs::read("cert.pem")?;

    //load certificate from file
    let cert = Certificate::from_pem(&buffer)?;

    // new builder with trusted root cert
    let mut builder = TlsConnector::builder();
    builder.add_root_certificate(cert);
    let connector = builder.build().unwrap();

    // connect with tcp stream
    let stream = TcpStream::connect(addr)?;
    let stream = connector.connect(&addr.ip().to_string(), stream)?;
    Ok(stream)
}
