#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! Bahndaten verlässlich und schnell in die Blockchain gepuffert - **Persistente Redundante Einheit für Langzeit-Logging über Blockchain**
//!
//! ## Overview
//!
//! `PrellBlock` is a lightweight logging blockchain, written in `Rust`, which is designed for datastorage purposes in a railway environment.
//! By using an execute-order-validate procedure it is assured, that data will be saved, even in case of a total failure of all but one redundant processing unit.
//! While working in full capactiy, data is stored and validated under byzantine fault tolerance. This project is carried out in cooperation with **Deutsche Bahn AG**.

use prellblock::api::{client, server};
use std::net::{SocketAddr, TcpListener};
use structopt::StructOpt;

// https://crates.io/crates/structopt

#[derive(StructOpt, Debug)]
struct Opt {
    /// The address on which to open the RPU communication server.
    #[structopt(short, long)]
    bind: Option<SocketAddr>,

    #[structopt(short, long)]
    peer: Option<SocketAddr>,
}

fn main() {
    pretty_env_logger::init();
    log::info!("Kitty =^.^=");

    let opt = Opt::from_args();
    log::debug!("Command line arguments: {:#?}", opt);

    // execute the server in a new thread
    let server_handle = opt.bind.map(|bind_addr| {
        std::thread::spawn(move || {
            let listener = TcpListener::bind(bind_addr).unwrap();
            let server = server::Server {};
            server.serve(&listener).unwrap();
        })
    });

    // execute the test client
    if let Some(peer_addr) = opt.peer {
        let mut client = client::Client::new(peer_addr);
        match client.send_request(prellblock::api::Ping()) {
            Err(err) => log::error!("Failed to send Ping: {}.", err),
            Ok(res) => log::debug!("Ping response: {:?}", res),
        }
        log::info!(
            "The sum is {:?}",
            client.send_request(prellblock::api::Add(100, 2))
        );
    }

    // wait for the server thread
    if let Some(server_handle) = server_handle {
        match server_handle.join() {
            Err(err) => log::error!("Server error occured: {:?}", err),
            Ok(()) => log::info!("No error occured. Going to hunt some mice. I meant *NICE*. Bye."),
        };
    }
}
