use super::BoxError;
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

pub async fn connect(addr: &SocketAddr) -> Result<StreamImpl, BoxError> {
    // open certificate file
    let buffer = fs::read("./certificates/ca/ca_prellblock-ca.cert")?;
    //load certificate from file
    let cert = Certificate::from_pem(&buffer)?;
    // new builder with trusted root cert
    let mut builder = TlsConnector::builder();
    builder.add_root_certificate(cert);
    let connector: AsyncTlsConnector = builder.build()?.into();

    // connect with tcp stream
    let stream = TcpStream::connect(addr).await?;
    let stream = connector.connect(&addr.ip().to_string(), stream).await?;
    Ok(stream)
}
