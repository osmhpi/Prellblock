//! A client for communicating between RPUs.

use super::RequestData;
use balise::client::Client;

/// A sender instance.
///
/// The sender keeps up a connection pool of open connections
/// for improved efficiency.
pub type Sender = Client<RequestData>;
