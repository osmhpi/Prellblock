//! Provide a server for gathering metrics with Prometheus.

use hyper::{
    header::CONTENT_TYPE,
    service::{make_service_fn, service_fn},
    Body, Error, Request, Response, Server,
};
use prometheus::{Encoder, TextEncoder};
use std::net::SocketAddr;

/// Start a HTTP-Server.
///
/// The server will listen on `address` and serve metrics.
pub async fn run_server(address: SocketAddr) -> Result<(), Error> {
    let serve_future = Server::bind(&address).serve(make_service_fn(|_| async {
        Ok::<_, hyper::Error>(service_fn(serve_request))
    }));

    serve_future.await
}

async fn serve_request(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    if req.uri() != "/metrics" {
        return Ok(Response::builder()
            .status(404)
            .body(Body::from("Not found"))
            .unwrap());
    }

    let encoder = TextEncoder::new();

    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();

    let response = Response::builder()
        .status(200)
        .header(CONTENT_TYPE, encoder.format_type())
        .body(Body::from(buffer))
        .unwrap();

    Ok(response)
}
