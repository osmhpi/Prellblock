#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! An example client used to simulate clients.

use pinxit::Identity;
use prellblock_client::Client;
use prellblock_client_api::{message, TransactionMessage};
use serde_json::json;
use std::net::SocketAddr;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(short, long)]
    peer: SocketAddr,
}

fn main() {
    pretty_env_logger::init();
    log::info!("Little Kitty =^.^=");

    let opt = Opt::from_args();
    log::debug!("Command line arguments: {:#?}", opt);

    // execute the test client
    let mut client = Client::new(opt.peer);
    match client.send_request(message::Ping) {
        Err(err) => log::error!("Failed to send Ping: {}.", err),
        Ok(res) => log::debug!("Ping response: {:?}", res),
    }

    let identity = Identity::generate();

    let key = "test".to_string();
    let value = json!({ "answer": 42 });

    let t_message = TransactionMessage {
        key: key.clone(),
        value: value.clone(),
    };

    let signature = identity.sign(t_message).unwrap();

    let peer_id = identity.id().clone();

    match client.send_request(message::SetValue(peer_id, key, value, signature)) {
        Err(err) => log::error!("Failed to send transaction: {}.", err),
        Ok(()) => log::debug!("Transaction ok!"),
    }
}
