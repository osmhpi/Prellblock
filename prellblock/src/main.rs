#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
//! Bahndaten verlässlich und schnell in die Blockchain gepuffert - **Persistente Redundante Einheit für Langzeit-Logging über Blockchain**
//!
//! ## Overview
//!
//! `PrellBlock` is a lightweight logging blockchain, written in `Rust`, which is designed for datastorage purposes in a railway environment.
//! By using an execute-order-validate procedure it is assured, that data will be saved, even in case of a total failure of all but one redundant processing unit.
//! While working in full capactiy, data is stored and validated under byzantine fault tolerance. This project is carried out in cooperation with **Deutsche Bahn AG**.

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

mod util;
use util::some_test;

fn main() {
    pretty_env_logger::init();
    trace!("leave no trace");
    debug!("debooging");
    info!("such information");
    warn!("o_O");
    error!("much error");
    some_test();
}

/// This is the **FOO FIGHTER**
pub fn foo() {
    println!("Malte says hi");
}
