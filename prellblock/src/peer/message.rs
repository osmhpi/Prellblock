//! All message types that can be sent between RPUs.

use super::{Request, RequestData};
use serde::{Deserialize, Serialize};

/// Add two numbers.
#[derive(Debug, Serialize, Deserialize)]
pub struct Add(pub usize, pub usize);

impl Request for Add {
    type Response = usize;
}

impl From<Add> for RequestData {
    fn from(v: Add) -> Self {
        Self::Add(v)
    }
}

// Add -> RequestData
// impl From<Add> for RequestData {}
// impl Into<RequestData> for Add {}

// let theAddition = Add(1,2);
// let req: RequestData = theAddition.into();

/// Subtract two numbers.
#[derive(Debug, Serialize, Deserialize)]
pub struct Sub(pub usize, pub usize);

impl Request for Sub {
    type Response = usize;
}

impl From<Sub> for RequestData {
    fn from(v: Sub) -> Self {
        Self::Sub(v)
    }
}

/// Ping Message.u8
#[derive(Debug, Serialize, Deserialize)]
pub struct Ping();

/// Subtract two numbers.
#[derive(Debug, Serialize, Deserialize)]
pub struct Pong;

impl Request for Ping {
    type Response = Pong;
}

impl From<Ping> for RequestData {
    fn from(v: Ping) -> Self {
        Self::Ping(v)
    }
}
