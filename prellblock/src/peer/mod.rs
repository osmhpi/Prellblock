//! Message types that can be used to communicate between RPUs.

// TODO: Define and use a macro to create the api spec boiler plate. (macro_rules!)
// See https://doc.rust-lang.org/book/ch19-06-macros.html
// and https://rustbyexample.com/macros.html

#![allow(clippy::wildcard_imports)]

mod calculator;
pub mod message;
mod receiver;
mod sender;

pub use calculator::Calculator;
use message::*;
pub use receiver::Receiver;
pub use sender::Sender;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;

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
