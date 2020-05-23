#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! A Library Crate for external Clients - Malte (TM)

mod client;
mod ffi;

pub use client::Client;
