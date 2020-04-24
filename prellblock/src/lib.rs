#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! Bahndaten verlässlich und schnell in die Blockchain gepuffert - **Persistente Redundante Einheit für Langzeit-Logging über Blockchain**
//!
//! ## Overview
//!
//! `PrellBlock` is a lightweight logging blockchain, written in `Rust`, which is designed for datastorage purposes in a railway environment.
//! By using an execute-order-validate procedure it is assured, that data will be saved, even in case of a total failure of all but one redundant processing unit.
//! While working in full capactiy, data is stored and validated under byzantine fault tolerance. This project is carried out in cooperation with **Deutsche Bahn AG**.

pub mod batcher;
pub mod block_storage;
pub mod consensus;
pub mod data_broadcaster;
pub mod data_storage;
pub mod peer;
pub mod permission_checker;
pub mod thread_group;
pub mod turi;
pub mod world_state;

// TODO: remove this sh**
type BoxError = Box<dyn std::error::Error + Send + Sync>;
