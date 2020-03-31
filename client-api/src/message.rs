//! All message types that can be sent between clients and RPUs.

use super::RequestData;
use balise::Request;
use serde::{Deserialize, Serialize};

/// Ping Message. See [`Pong`](struct.Pong.html).
#[derive(Debug, Serialize, Deserialize)]
pub struct Ping;

/// Play ping pong. See [`Ping`](struct.Ping.html).
#[derive(Debug, Serialize, Deserialize)]
pub struct Pong;

impl Request<RequestData> for Ping {
    type Response = Pong;
}

impl From<Ping> for RequestData {
    fn from(v: Ping) -> Self {
        Self::Ping(v)
    }
}
