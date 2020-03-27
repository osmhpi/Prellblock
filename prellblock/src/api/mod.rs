//! Message types that can be used to communicate between RPUs.

// TODO: Define and use a macro to create the api spec boiler plate. (macro_rules!)
// See https://doc.rust-lang.org/book/ch19-06-macros.html
// and https://rustbyexample.com/macros.html

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;

pub mod client;
pub mod server;

/// A request to the API always has a specific response type.
pub trait Request: Serialize + Into<RequestData> + Debug {
    /// The type of the response.
    type Response: Serialize + DeserializeOwned + Debug;
}

/// One of the requests.
#[allow(missing_docs)]
#[derive(Debug, Serialize, Deserialize)]
pub enum RequestData {
    Add(Add),
    Sub(Sub),
    Ping(Ping),
}

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
