//! A client for communicating between RPUs.

use balise::client;
use prellblock_client_api::ClientMessage;

/// A Client Instance.
///
/// Used for Communication between Client Entities and RPU Servers.
pub type Client = client::Client<ClientMessage>;
