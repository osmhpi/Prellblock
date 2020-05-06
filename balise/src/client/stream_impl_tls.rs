use crate::Error;
use lazy_static::lazy_static;
use native_tls::{Certificate, TlsConnector};
use std::{fs, net::SocketAddr};
use tokio::net::TcpStream;
use tokio_tls::{TlsConnector as AsyncTlsConnector, TlsStream};

pub type StreamImpl = TlsStream<TcpStream>;

impl<'a> super::StreamGuard<'a> {
    pub fn tcp_stream(&self) -> &TcpStream {
        self.stream.as_ref().unwrap().get_ref()
    }
}

lazy_static! {
    static ref CONNECTOR: AsyncTlsConnector = {
        // open certificate file
        let buffer = fs::read("./certificates/ca/ca_prellblock-ca.cert").unwrap();
        //load certificate from file
        let cert = Certificate::from_pem(&buffer).unwrap();
        // new builder with trusted root cert
        let mut builder = TlsConnector::builder();
        builder.add_root_certificate(cert);
        builder.build().unwrap().into()
    };
}

pub async fn connect(addr: &SocketAddr) -> Result<StreamImpl, Error> {
    // connect with tcp stream
    let stream = TcpStream::connect(addr).await?;
    let stream = CONNECTOR.connect(&addr.ip().to_string(), stream).await?;
    Ok(stream)
}
