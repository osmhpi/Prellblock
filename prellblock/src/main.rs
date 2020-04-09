#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! Bahndaten verlässlich und schnell in die Blockchain gepuffert - **Persistente Redundante Einheit für Langzeit-Logging über Blockchain**
//!
//! ## Overview
//!
//! `PrellBlock` is a lightweight logging blockchain, written in `Rust`, which is designed for datastorage purposes in a railway environment.
//! By using an execute-order-validate procedure it is assured, that data will be saved, even in case of a total failure of all but one redundant processing unit.
//! While working in full capactiy, data is stored and validated under byzantine fault tolerance. This project is carried out in cooperation with **Deutsche Bahn AG**.

use prellblock::{
    batcher::Batcher,
    data_broadcaster::Broadcaster,
    data_storage::DataStorage,
    peer::{Calculator, PeerInbox, Receiver},
    thread_group::ThreadGroup,
    turi::Turi,
};
use serde::Deserialize;
use std::{
    fs,
    net::{SocketAddr, TcpListener},
    sync::Arc,
};
use structopt::StructOpt;

// https://crates.io/crates/structopt

#[derive(StructOpt, Debug)]
struct Opt {
    /// The identity name to load from config.toml file.
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    rpu: Vec<RpuConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct RpuConfig {
    name: String,
    peer_id: String,
    peer_address: SocketAddr,
    turi_address: SocketAddr,
}

#[derive(Debug, Clone, Deserialize)]
struct RpuPrivateConfig {
    identity: String, // pinxit::Identity (hex -> .key)
    tls_id: String,   // native_tls::Identity (pkcs12 -> .pfx)
}

fn main() {
    pretty_env_logger::init();
    log::info!("Kitty =^.^=");

    let opt = Opt::from_args();
    log::debug!("Command line arguments: {:#?}", opt);

    // load and parse config
    let config_data = fs::read_to_string("./config/config.toml").unwrap();
    let config: Config = toml::from_str(&config_data).unwrap();
    let public_config = config
        .rpu
        .iter()
        .find(|rpu_config| rpu_config.name == opt.name)
        .unwrap()
        .clone();
    let private_config_data =
        fs::read_to_string(format!("./config/{0}/{0}.toml", opt.name)).unwrap();
    let private_config: RpuPrivateConfig = toml::from_str(&private_config_data).unwrap();
    // join handles of all threads

    let mut thread_group = ThreadGroup::new();

    let peer_addresses: Vec<SocketAddr> = config
        .rpu
        .iter()
        .map(|rpu_config| rpu_config.peer_address)
        .collect();

    let broadcaster = Broadcaster::new(peer_addresses);
    let broadcaster = Arc::new(broadcaster);

    let batcher = Batcher::new(broadcaster);
    let batcher = Arc::new(batcher);

    // execute the turi in a new thread
    {
        let public_config = public_config.clone();
        let private_config = private_config.clone();

        thread_group.spawn(
            format!("Turi ({})", public_config.turi_address),
            move || {
                let listener = TcpListener::bind(public_config.turi_address)?;
                let turi = Turi::new(private_config.tls_id, batcher);
                turi.serve(&listener)
            },
        );
    }

    let storage = DataStorage::new(&format!("./data/{}", opt.name)).unwrap();
    let storage = Arc::new(storage);

    let calculator = Calculator::new();
    let calculator = Arc::new(calculator.into());

    let peer_inbox = PeerInbox::new(calculator, storage);
    let peer_inbox = Arc::new(peer_inbox);

    // execute the receiver in a new thread
    thread_group.spawn(
        format!("Peer Receiver ({})", public_config.peer_address),
        move || {
            let listener = TcpListener::bind(public_config.peer_address)?;
            let receiver = Receiver::new(private_config.tls_id, peer_inbox);
            receiver.serve(&listener)
        },
    );

    // wait for all threads
    thread_group.join_and_log();
    log::info!("Going to hunt some mice. I meant *NICE*. Bye.");
}
