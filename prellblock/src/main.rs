#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! Bahndaten verlässlich und schnell in die Blockchain gepuffert - **Persistente Redundante Einheit für Langzeit-Logging über Blockchain**
//!
//! ## Overview
//!
//! `PrellBlock` is a lightweight logging blockchain, written in `Rust`, which is designed for datastorage purposes in a railway environment.
//! By using an execute-order-validate procedure it is assured, that data will be saved, even in case of a total failure of all but one redundant processing unit.
//! While working in full capactiy, data is stored and validated under byzantine fault tolerance. This project is carried out in cooperation with **Deutsche Bahn AG**.

use balise::server::TlsIdentity;
use futures::future;
use prellblock::{
    batcher::Batcher,
    block_storage::BlockStorage,
    consensus::Consensus,
    data_broadcaster::Broadcaster,
    data_storage::DataStorage,
    peer::{Calculator, PeerInbox, Receiver},
    reader::Reader,
    transaction_checker::TransactionChecker,
    turi::Turi,
    world_state::WorldStateService,
};
use prellblock_client_api::consensus::GenesisTransactions;
use serde::Deserialize;
use std::{env, fs, io, net::SocketAddr, sync::Arc};
use structopt::StructOpt;
use tokio::net::TcpListener;

// https://crates.io/crates/structopt

#[derive(StructOpt, Debug)]
struct Opt {
    /// The path to the configuration file.
    config: String,
    /// The path to the genesis transactions file (only needed for the first start).
    genesis_transactions: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RpuPrivateConfig {
    identity: String,   // pinxit::Identity (hex -> .key)
    tls_id: String,     // native_tls::Identity (pkcs12 -> .pfx)
    block_path: String, // path to `BlockStorage` directory
    data_path: String,  // path to `DataStorage` directory
    turi_address: SocketAddr,
    peer_address: SocketAddr,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Kitty =^.^=");

    let opt = Opt::from_args();
    log::debug!("Command line arguments: {:#?}", opt);

    // load and parse config
    let private_config_data = fs::read_to_string(opt.config).unwrap();
    let private_config: RpuPrivateConfig = toml::from_str(&private_config_data).unwrap();

    // load genesis block (if a path is given)
    let genesis_transactions = if let Some(genesis_transactions) = opt.genesis_transactions {
        let genesis_transactions_data = fs::read_to_string(genesis_transactions).unwrap();
        let genesis_transactions: GenesisTransactions =
            serde_yaml::from_str(&genesis_transactions_data).unwrap();
        Some(genesis_transactions)
    } else {
        None
    };

    let hex_identity =
        fs::read_to_string(&private_config.identity).expect("Could not load identity file.");
    let identity = hex_identity.parse().expect("Identity could not be loaded.");

    let block_storage =
        BlockStorage::new(&private_config.block_path, genesis_transactions).unwrap();
    let world_state = WorldStateService::from_block_storage(&block_storage).unwrap();

    let consensus = Consensus::new(identity, block_storage.clone(), world_state.clone()).await;

    let broadcaster = Broadcaster::new(world_state.clone());
    let broadcaster = Arc::new(broadcaster);

    let batcher = Batcher::new(broadcaster);

    let reader = Reader::new(block_storage, world_state.clone());

    let transaction_checker = TransactionChecker::new(world_state);

    // execute the turi in a new thread
    let turi_task = {
        let private_config = private_config.clone();
        let transaction_checker = transaction_checker.clone();

        tokio::spawn(async move {
            let tls_identity = load_identity_from_env(private_config.tls_id).await?;
            let mut listener = TcpListener::bind(private_config.turi_address).await?;
            let turi = Turi::new(tls_identity, batcher, reader, transaction_checker);
            turi.serve(&mut listener).await
        })
    };

    let data_storage = DataStorage::new(&private_config.data_path).unwrap();
    let data_storage = Arc::new(data_storage);

    let calculator = Calculator::new();
    let calculator = Arc::new(calculator.into());

    let peer_inbox = PeerInbox::new(calculator, data_storage, consensus, transaction_checker);
    let peer_inbox = Arc::new(peer_inbox);

    // execute the receiver in a new thread
    let peer_receiver_task = tokio::spawn(async move {
        let tls_identity = load_identity_from_env(private_config.tls_id).await?;
        let mut listener = TcpListener::bind(private_config.peer_address).await?;
        let receiver = Receiver::new(tls_identity, peer_inbox);
        receiver.serve(&mut listener).await
    });

    // wait for all tasks
    future::join(
        async move {
            log::error!("Turi ended: {:?}", turi_task.await);
        },
        async move {
            log::error!("Peer recceiver ended: {:?}", peer_receiver_task.await);
        },
    )
    .await;
    log::info!("Going to hunt some mice. I meant *NICE*. Bye.");
}

async fn load_identity_from_env(tls_identity_path: String) -> Result<TlsIdentity, io::Error> {
    let password = env::var("TLS_PASSWORD").unwrap_or_else(|_| "prellblock".to_string());
    balise::server::load_identity(tls_identity_path, &password).await
}
