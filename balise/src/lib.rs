#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]
#![allow(clippy::needless_doctest_main)]

//! An Eurobalise is a specific variant of a balise being a transponder placed between the rails of a railway.
//! These balises constitute an integral part of the European Train Control System,
//! where they serve as "beacons" giving the exact location of a train
//! as well as transmitting signalling information in a digital telegram to the train.
//!
//! ## Example
//! ```no_run
//! use serde::{Deserialize, Serialize};
//!
//! // ---------------- Define API definition ----------------
//! use balise::define_api;
//!
//! #[derive(Debug, Serialize, Deserialize)]
//! pub struct Pong;
//!
//! define_api! {
//!     mod ping_message; // give a module name for all requests
//!     pub enum PingAPIMessage {
//!         Ping => Pong,
//!         Add(usize, usize) => usize,
//!     }
//! }
//!
//! // ---------------- Define API client ----------------
//! use balise::client::Client;
//! type PingAPIClient = Client<PingAPIMessage>;
//!
//! // ---------------- Define API server ----------------
//! use balise::{
//!     handle_fn,
//!     server::{Handler, Response, Server},
//!     Request,
//! };
//! use std::net::{SocketAddr, TcpListener};
//!
//! type BoxError = Box<dyn std::error::Error + Send + Sync>;
//!
//! #[derive(Clone)]
//! struct PingAPIServer;
//!
//! impl PingAPIServer {
//!     /// The main server loop.
//!     pub fn serve(self, listener: &TcpListener) -> Result<(), BoxError> {
//!         #[cfg(not(feature = "tls"))]
//!         {
//!             let server = Server::new(self);
//!             server.serve(listener);
//!         }
//!
//!         #[cfg(feature = "tls")]
//!         {
//!             match Server::new(self, "path_to.pfx".to_string(), "password") {
//!                 Ok(server) => server.serve(listener),
//!                 Err(err) => panic!("Could not start server: {}.", err),
//!             }
//!         }
//!     }
//! }
//!
//! impl Handler<PingAPIMessage> for PingAPIServer {
//!     handle_fn!(self, PingAPIMessage, {
//!         Ping(_) => Ok(Pong),
//!         Add(params) => Ok(params.0 + params.1),
//!     });
//! }
//!
//! // ---------------- Start server and send request ----------------
//!
//! fn main() {
//!     let bind_addr = "127.0.0.1:0"; // replace 0 with a useful port
//!     let listener = TcpListener::bind(bind_addr).unwrap();
//!     let peer_addr = listener.local_addr().unwrap(); // address with allocated port
//!
//!     std::thread::spawn(move || {
//!         let server = PingAPIServer;
//!         server.serve(&listener).unwrap();
//!     });
//!
//!     // use a client to execute some requests
//!     let mut client = PingAPIClient::new(peer_addr);
//!     match client.send_request(ping_message::Ping) {
//!         Err(err) => log::error!("Failed to send Ping: {}", err),
//!         Ok(res) => log::debug!("Ping response: {:?}", res),
//!     }
//! }
//! ```

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

mod macros;
mod stream;

pub use stream::Stream;

use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A request to the API always has a specific response type.
///
/// All Variants of a message enum `T` must implement this trait
/// (is done automatically when using the [`define_api!`](macro.define_api.html)-macro).
/// This allows a message `M` which implements `Request<T>` to be converted to one of the enum variants (via `Into<T>`).
/// And the implementation can ensure that the response is of type `M::Response`.
pub trait Request<T>: Serialize + Into<T> + Debug + Clone + Send + 'static {
    /// The type of the response.
    type Response: Serialize + DeserializeOwned + Debug + Send + 'static;

    /// Call the request handler and encode the response.
    #[cfg(feature = "server")]
    fn handle(
        self,
        handler: impl FnOnce(Self) -> Result<Self::Response, BoxError>,
    ) -> Result<server::Response, BoxError> {
        let res = handler(self)?;
        Ok(server::Response(serde_json::to_value(&res)?))
    }
}
