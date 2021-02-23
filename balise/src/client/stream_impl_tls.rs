use crate::{Error, Address};
use lazy_static::lazy_static;
use native_tls::{Certificate, TlsConnector};
use std::{env, fs};
use tokio::net::{TcpStream};
use tokio_native_tls::{TlsConnector as AsyncTlsConnector, TlsStream};

pub type StreamImpl = TlsStream<TcpStream>;

impl<'a> super::StreamGuard<'a> {
    pub fn tcp_stream(&self) -> &TcpStream {
        self.stream.as_ref().unwrap().get_ref().get_ref().get_ref()
    }
}

lazy_static! {
    static ref CONNECTOR: AsyncTlsConnector = {
        // open certificate file
        let ca_cert_path = env::var("CA_CERT_PATH").unwrap_or_else(|_| "./config/ca/ca-certificate.pem".to_string());
        let buffer = fs::read(ca_cert_path).unwrap();
        // load certificate from file
        let cert = Certificate::from_pem(&buffer).unwrap();
        // new builder with trusted root cert
        let mut builder = TlsConnector::builder();
        builder.add_root_certificate(cert);
        builder.build().unwrap().into()
    };
}

pub async fn connect(addr: &Address) -> Result<StreamImpl, Error> {
    // connect with tcp stream
    let stream = TcpStream::connect((addr.host.to_string(), addr.port)).await?;
    let stream = CONNECTOR.connect(&addr.host.to_string(), stream).await?;
    Ok(stream)
}
