#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! Library Crate used for Communication between external Clients and internal RPUs.

#![allow(clippy::wildcard_imports)]

pub mod message;

use message::*;
use serde::{Deserialize, Serialize};

/// One of the requests.
#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
pub enum RequestData {
    Ping(Ping),
}
