#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_errors_doc, clippy::similar_names)]

//! An example client used to simulate clients.

use pinxit::Identity;
use prellblock_client::Client;
use prellblock_client_api::account_permissions::Permissions;
use rand::{
    rngs::{OsRng, StdRng},
    seq::SliceRandom,
    thread_rng, RngCore, SeedableRng,
};
use serde::Deserialize;
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
    /// Benchmark the blockchain.
    #[structopt(name = "bench")]
    Benchmark {
        /// The name of the RPU to benchmark.
        rpu_name: String,
        /// The key to use for saving benchmark generated data.
        key: String,
        /// The number of transactions to send
        transactions: u32,
        /// The number of bytes each transaction's payload should have.
        #[structopt(short, long, default_value = "8")]
        size: usize,
        /// The number of workers (clients) to use simultaneously.
        #[structopt(short, long, default_value = "1")]
        workers: usize,
    },
    /// Update the permissions for a given account.
    #[structopt(name = "update")]
    UpdateAccount {
        /// The id of the account to update.
        id: String,
        /// The filepath to a yaml-file cotaining the accounts permissions.
        permission_file: String,
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

#[tokio::main]
async fn main() {
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
            let identity = "406ed6170c8672e18707fb7512acf3c9dbfc6e5ad267d9a57b9c486a94d99dcc"
                .parse()
                .unwrap();
            let mut client = Client::new(turi_address, identity);

            match client.send_key_value(key, value).await {
                Err(err) => log::error!("Failed to send transaction: {}", err),
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

            let mut worker_handles = Vec::new();
            for _ in 0..workers {
                let key = key.clone();
                worker_handles.push(tokio::spawn(async move {
                    let mut rng = StdRng::from_rng(OsRng {}).unwrap();
                    let identity =
                        "406ed6170c8672e18707fb7512acf3c9dbfc6e5ad267d9a57b9c486a94d99dcc"
                            .parse()
                            .unwrap();
                    let mut client = Client::new(turi_address, identity);

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
                        match client.send_key_value(key, value).await {
                            Err(err) => log::error!("Failed to send transaction: {}", err),
                            Ok(()) => log::debug!("Transaction ok!"),
                        }
                    }
                    start.elapsed()
                }));
            }

            for (n, worker) in worker_handles.into_iter().enumerate() {
                if let Ok(time_diff) = worker.await {
                    let avg_time_per_tx = time_diff / transactions;
                    let avg_tps = 1.0 / avg_time_per_tx.as_secs_f64();
                    log::info!(
                        "--------------------------------------------------------------------------------"
                    );
                    log::info!("Finished benchmark with worker {}.", n);
                    log::info!("Number of transactions: {}", transactions);
                    log::info!("Transaction size:       {} bytes", size);
                    log::info!(
                        "Sum of sent payload:    {} bytes",
                        size * transactions as usize
                    );
                    log::info!("Duration:               {:?}", time_diff);
                    log::info!("Transaction time:       {:?}", avg_time_per_tx);
                    log::info!("TPS (averaged):         {}", avg_tps);
                } else {
                    log::error!("Failed to benchmark with worker {}", n);
                }
            }
        }
        Command::UpdateAccount {
            id,
            permission_file,
        } => {
            let mut rng = thread_rng();
            let turi_address = config.rpu.choose(&mut rng).unwrap().turi_address;

            // matching peerid is: cb932f482dc138a76c6f679862aa3692e08c140284967f687c1eaf75fd97f1bc
            let identity: Identity =
                "03d738c972f37a6fd9b33278ac0c50236e45637bcd5aeee82d8323655257d256"
                    .parse()
                    .unwrap();

            let mut client = Client::new(turi_address, identity);

            // TestCLI: 256cdb0197402705f96d39eab7dd3d47a39cb75673a58852d83f666973d80e01
            let id = id.parse().expect("Invalid account id given");

            // Read `Permissions` from the given file.
            let permission_file_content =
                fs::read_to_string(permission_file).expect("Could not read permission file");
            let permissions: Permissions = serde_yaml::from_str(&permission_file_content)
                .expect("Invalid permission file content");

            match client.update_account(id, permissions).await {
                Err(err) => log::error!("Failed to send transaction: {}", err),
                Ok(()) => log::debug!("Transaction ok!"),
            }
        }
    }
}
