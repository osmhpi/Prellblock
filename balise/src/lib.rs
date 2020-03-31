#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! An Eurobalise is a specific variant of a balise being a transponder placed between the rails of a railway.
//! These balises constitute an integral part of the European Train Control System,
//! where they serve as "beacons" giving the exact location of a train
//! as well as transmitting signalling information in a digital telegram to the train.

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

mod stream;

pub use stream::Stream;

use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

/// A request to the API always has a specific response type.
pub trait Request<T>: Serialize + Into<T> + Debug {
    /// The type of the response.
    type Response: Serialize + DeserializeOwned + Debug;
}
