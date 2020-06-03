#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::future_not_send,
    clippy::missing_errors_doc,
    clippy::similar_names
)]

//! An example client used to simulate clients.

mod cli;

use cli::prelude::*;
use pinxit::Identity;
use prellblock_client::{account::Permissions, Client};
use rand::{
    rngs::{OsRng, StdRng},
    seq::SliceRandom,
    thread_rng, RngCore, SeedableRng,
};
use std::{fs, str, time::Instant};
use structopt::StructOpt;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Little Kitty =^.^=");

    let opt = Opt::from_args();
    log::debug!("Command line arguments: {:#?}", opt);

    match opt.cmd {
        Cmd::Set(cmd) => main_set(cmd).await,
        Cmd::Benchmark(cmd) => main_benchmark(cmd).await,
        Cmd::UpdateAccount(cmd) => main_update_account(cmd).await,
    }
}

async fn main_set(cmd: cmd::Set) {
    let cmd::Set { key, value } = cmd;

    let mut rng = thread_rng();
    let turi_address = Config::load().rpu.choose(&mut rng).unwrap().turi_address;
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

async fn main_benchmark(cmd: cmd::Benchmark) {
    let cmd::Benchmark {
        rpu_name,
        key,
        transactions,
        size,
        workers,
    } = cmd;

    let turi_address = Config::load()
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
            let identity = "406ed6170c8672e18707fb7512acf3c9dbfc6e5ad267d9a57b9c486a94d99dcc"
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

async fn main_update_account(cmd: cmd::UpdateAccount) {
    let cmd::UpdateAccount {
        id,
        permission_file,
    } = cmd;

    let mut rng = thread_rng();
    let turi_address = Config::load().rpu.choose(&mut rng).unwrap().turi_address;

    // matching peerid is: cb932f482dc138a76c6f679862aa3692e08c140284967f687c1eaf75fd97f1bc
    let identity: Identity = "03d738c972f37a6fd9b33278ac0c50236e45637bcd5aeee82d8323655257d256"
        .parse()
        .unwrap();

    let mut client = Client::new(turi_address, identity);

    // TestCLI: 256cdb0197402705f96d39eab7dd3d47a39cb75673a58852d83f666973d80e01
    let id = id.parse().expect("Invalid account id given");

    // Read `Permissions` from the given file.
    let permission_file_content =
        fs::read_to_string(permission_file).expect("Could not read permission file");
    let permissions: Permissions =
        serde_yaml::from_str(&permission_file_content).expect("Invalid permission file content");

    match client.update_account(id, permissions).await {
        Err(err) => log::error!("Failed to send transaction: {}", err),
        Ok(()) => log::debug!("Transaction ok!"),
    }
}
