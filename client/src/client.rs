//! A client for communicating between RPUs.

use balise::client;
use client_api::RequestData;

/// A Client Instance.
///
/// Used for Communication between Client Entities and RPU Servers.
pub type Client = client::Client<RequestData>;
