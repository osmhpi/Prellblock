//! Provide a server for gathering metrics with Prometheus.
//!
//! The following metrics will be gathered:
//! - [x] number of (valid and invalid) transactions in Turi
//! - [x] time in consensus queue
//! - [x] time to data storage (based on client transaction timestamps)
//! - [x] time to block storage (based on client transaction timestamps)
//! - [x] number of (valid and invalid) transactions in peer inbox / data storage
//! - [x] size of data storage
//! - [x] number of blocks in block storage
//! - [x] number of transactions in block storage
//! - [x] size of block storage (<https://docs.rs/sled/0.32.0-rc1/sled/struct.Db.html#method.size_on_disk>)
//! - [x] leader term
//! - [x] time of last leader change
//! - [x] time of last prepare message
//! - [x] time of last append message
//! - [x] time of last commit message

use hyper::{
    header::CONTENT_TYPE,
    service::{make_service_fn, service_fn},
    Body, Error, Request, Response, Server,
};
use prometheus::{Encoder, TextEncoder};
use std::net::SocketAddr;

/// Start a HTTP-Server.
///
/// The server will listen on `address` and serve metrics under `/metrics`.
pub async fn run_server(address: SocketAddr) -> Result<(), Error> {
    let serve_future = Server::bind(&address).serve(make_service_fn(|_| async {
        Ok::<_, hyper::Error>(service_fn(serve_request))
    }));

    serve_future.await
}

async fn serve_request(req: Request<Body>) -> Result<Response<Body>, hyper::http::Error> {
    if req.uri() != "/metrics" {
        return Ok(Response::builder()
            .status(404)
            .body(Body::from("Not found"))
            .unwrap());
    }

    let encoder = TextEncoder::new();

    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    if let Err(err) = encoder.encode(&metric_families, &mut buffer) {
        let response = Response::builder()
            .status(500)
            .body(Body::from(err.to_string()))?;
        return Ok(response);
    }

    let response = Response::builder()
        .status(200)
        .header(CONTENT_TYPE, encoder.format_type())
        .body(buffer.into())?;

    Ok(response)
}
