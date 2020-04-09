#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! An example client used to simulate clients.

use pinxit::{Identity, Signable};
use prellblock_client::Client;
use prellblock_client_api::{message, Transaction};
use rand::{seq::SliceRandom, thread_rng, RngCore};
use serde::Deserialize;
use serde_json::json;
use std::{fs, net::SocketAddr, str, time::Instant};
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
    /// Benchmark the blockchain
    #[structopt(name = "bench")]
    Benchmark {
        /// The name of the RPU to benchmark.
        rpu_name: String,
        /// The key to use for saving benchmark generated data.
        key: String,
        /// The number of transactions to send
        transactions: usize,
        /// The number of bytes each transaction's payload should have.
        #[structopt(short, long, default_value = "8")]
        size: usize,
        /// The number of workers (clients) to use simultaneously.
        #[structopt(short, long, default_value = "1")]
        workers: usize,
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

            let identity = Identity::from_hex(
                "406ed6170c8672e18707fb7512acf3c9dbfc6e5ad267d9a57b9c486a94d99dcc",
            )
            .unwrap();
            let value = json!(value);

            let transaction = Transaction::KeyValue { key, value }
                .sign(&identity)
                .unwrap();

            let peer_id = identity.id().clone();

            match client.send_request(message::Execute(peer_id, transaction)) {
                Err(err) => log::error!("Failed to send transaction: {}.", err),
                Ok(()) => log::debug!("Transaction ok!"),
            }
        }
        Command::Benchmark {
            rpu_name,
            key,
            transactions,
            size,
            workers,
        } => {
            let turi_address = config
                .rpu
                .iter()
                .find(|rpu| rpu.name == rpu_name)
                .unwrap()
                .turi_address;
            // execute the test client
            let mut worker_handles = Vec::new();
            for _ in 0..workers {
                let key = key.clone();
                worker_handles.push(std::thread::spawn(move || {
                    let mut rng = thread_rng();
                    let mut client = Client::new(turi_address);
                    let identity = Identity::from_hex(
                        "406ed6170c8672e18707fb7512acf3c9dbfc6e5ad267d9a57b9c486a94d99dcc",
                    )
                    .unwrap();

                    let start = Instant::now();
                    let half_size = (size + 1) / 2;
                    let mut bytes = vec![0; half_size];
                    let mut value = vec![0; half_size * 2];
                    for _ in 0..transactions {
                        let key = key.clone();
                        // generate random data (hex)
                        rng.fill_bytes(&mut bytes);
                        hex::encode_to_slice(&bytes, &mut value).unwrap();
                        let value = str::from_utf8(&value[..size]).unwrap();
                        let value = json!(value);
                        let transaction = Transaction::KeyValue { key, value }
                            .sign(&identity)
                            .unwrap();
                        let peer_id = identity.id().clone();
                        match client.send_request(message::Execute(peer_id, transaction)) {
                            Err(err) => log::error!("Failed to send transaction: {}.", err),
                            Ok(()) => log::debug!("Transaction ok!"),
                        }
                    }
                    let time_diff = start.elapsed();
                    time_diff
                }));
            }

            for (n, worker) in worker_handles.into_iter().enumerate() {
                match worker.join() {
                    Ok(time_diff) => {
                        let avg_time_per_tx = time_diff.div_f64(transactions as f64);
                        let avg_tps = 1.0 / avg_time_per_tx.as_secs_f64();
                        log::info!(
                            "--------------------------------------------------------------------------------"
                        );
                        log::info!("Finished benchmark with worker {}.", n);
                        log::info!("Number of transactions: {}", transactions);
                        log::info!("Transaction size:       {} bytes", size);
                        log::info!("Sum of sent payload:    {} bytes", size * transactions);
                        log::info!("Duration:               {:?}", time_diff);
                        log::info!("Transaction time:       {:?}", avg_time_per_tx);
                        log::info!("TPS (averaged):         {}", avg_tps);
                    }
                    Err(_) => log::error!("Failed to benchmark with worker {}", n),
                }
            }
        }
    }
}
