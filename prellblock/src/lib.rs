#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_errors_doc,
    clippy::similar_names,
    clippy::doc_markdown
)]

//! Bahndaten verlässlich und schnell in die Blockchain gepuffert - **Persistente Redundante Einheit für Langzeit-Logging über Blockchain**
//!
//! ## Overview
//!
//! `Prellblock` is a lightweight (private) logging blockchain, written in `Rust`, which is designed for datastorage purposes in a railway environment.
//! By using an replicate-order-validate-execute procedure it is assured, that data will be saved, even in case of a total failure of all but one redundant processing unit.
//! While working in full capactiy, data is stored and validated under byzantine fault tolerance. This project is carried out in cooperation with **Deutsche Bahn AG represented by DB Systel GmbH**.

use serde::{Deserialize, Serialize};

pub mod batcher;
pub mod block_storage;
pub mod consensus;
pub mod data_broadcaster;
pub mod data_storage;
pub mod peer;
pub mod reader;
pub mod time;
pub mod transaction_checker;
pub mod turi;
pub mod world_state;

if_monitoring! {
    pub mod prometheus;
}

// TODO: remove this sh** lmao yeet
type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The Configuration for an RPU.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpuPrivateConfig {
    /// The `PeerId` of the RPU.
    pub identity: String, // pinxit::Identity (hex -> .key)
    /// The TLS identityfile path.
    pub tls_id: String, // native_tls::Identity (pkcs12 -> .pfx)
    /// The path to the directory for the `BlockStorage`.
    pub block_path: String,
    /// The path to the directory for the `DataStorage`.
    pub data_path: String,
}
/// Only compile this code when the `monitoring` feature is enabled.
/// ```
/// use prellblock::if_monitoring;
///
/// if_monitoring! {
///     pub use std::collections::HashMap;
/// }
/// // or
/// if_monitoring! {{
///     println!("Monitoring enabled!");
/// }};
/// ```
#[macro_export]
macro_rules! if_monitoring {
    ($block:block) => {
        #[cfg(feature = "monitoring")]
        $block
    };
    ($($item:item)*) => {
        $(
            #[cfg(feature = "monitoring")]
            $item
        )*
    };
}
