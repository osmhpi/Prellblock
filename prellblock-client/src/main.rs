#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! An example client used to simulate clients.

use prellblock_client::Client;
use prellblock_client_api::message;
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
}
