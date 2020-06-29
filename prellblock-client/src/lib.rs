#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! A Library Crate for external Clients - Malte (TM)

mod client;

pub use client::Client;
pub use pinxit::PeerId;
pub use prellblock_client_api::{account, consensus, Filter, Query, Span};
