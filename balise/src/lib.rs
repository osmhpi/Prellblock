#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]
#![allow(clippy::future_not_send)]

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
//!     handler,
//!     server,
//!     server::{Response, Server},
//!     Address,
//!     Error,
//!     Host,
//!     Request,
//! };
//! use std::net::SocketAddr;
//! use tokio::net::TcpListener;
//!
//! #[derive(Clone)]
//! struct PingAPIServer;
//!
//! impl PingAPIServer {
//!     /// The main server loop.
//!     pub async fn serve(self, listener: &mut TcpListener) -> Result<(), Error> {
//!         let handler = handler!(PingAPIMessage, {
//!             Ping(_) => Ok(Pong),
//!             Add(params) => Ok(params.0 + params.1),
//!         });
//!
//!         #[cfg(not(feature = "tls"))]
//!         {
//!             let server = Server::new(handler);
//!             server.serve(listener).await
//!         }
//!
//!         #[cfg(feature = "tls")]
//!         {
//!             let tls_identity = server::load_identity("path_to.pfx".to_string(), "password").await.unwrap();
//!             match Server::new(handler, tls_identity) {
//!                 Ok(server) => server.serve(listener).await,
//!                 Err(err) => panic!("Could not start server: {}.", err),
//!             }
//!         }
//!
//!     }
//! }
//!
//! // ---------------- Start server and send request ----------------
//! #[tokio::main]
//! async fn main() {
//!     let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap(); // replace 0 with a useful port
//!     let mut listener = TcpListener::bind(bind_addr).await.unwrap();
//!     let peer_addr = listener.local_addr().unwrap(); // address with allocated port
//!     let peer_addr: Address = peer_addr.to_string().parse().unwrap();
//!
//!     tokio::spawn(async move {
//!         let server = PingAPIServer;
//!         server.serve(&mut listener).await.unwrap();
//!     });
//!
//!     // use a client to execute some requests
//!     let mut client = PingAPIClient::new(peer_addr);
//!     match client.send_request(ping_message::Ping).await {
//!         Err(err) => log::error!("Failed to send Ping: {}", err),
//!         Ok(res) => log::debug!("Ping response: {:?}", res),
//!     }
//! }
//! ```

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

mod address;
mod error;
mod macros;
mod stream;

pub use address::{Address, Host};
pub use error::Error;
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
}
