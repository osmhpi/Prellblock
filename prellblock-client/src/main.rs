#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! An example client used to simulate clients.

use pinxit::Identity;
use prellblock_client::Client;
use prellblock_client_api::{message, TransactionMessage};
use rand::{seq::SliceRandom, thread_rng};
use serde::Deserialize;
use serde_json::json;
use std::{fs, net::SocketAddr};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(StructOpt, Debug)]
enum Command {
    /// Transaction to set a key to a value.
    Set {
        /// The key of this transaction.
        key: String,
        /// The value of the corresponding key.
        value: String,
    },
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

fn main() {
    pretty_env_logger::init();
    log::info!("Little Kitty =^.^=");

    let opt = Opt::from_args();
    log::debug!("Command line arguments: {:#?}", opt);

    let config_data = fs::read_to_string("./config/config.toml").unwrap();
    let config: Config = toml::from_str(&config_data).unwrap();

    match opt.cmd {
        Command::Set { key, value } => {
            let mut rng = thread_rng();
            let turi_address = config.rpu.choose(&mut rng).unwrap().turi_address;
            // execute the test client
            let mut client = Client::new(turi_address);

            let identity = Identity::generate();
            let value = json!(value);

            let t_message = TransactionMessage {
                key: &key,
                value: &value,
            };

            let signature = identity.sign(t_message).unwrap();
            let peer_id = identity.id().clone();

            match client.send_request(message::SetValue(peer_id, key, value, signature)) {
                Err(err) => log::error!("Failed to send transaction: {}.", err),
                Ok(()) => log::debug!("Transaction ok!"),
            }
        }
    }
}
