#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::future_not_send,
    clippy::missing_errors_doc,
    clippy::similar_names
)]

//! An example client used to simulate clients.

mod cli;

use balise::Address;
use cli::prelude::*;
use prellblock_client::{account::Permissions, Client, Query};
use rand::{
    rngs::{OsRng, StdRng},
    RngCore, SeedableRng,
};
use std::{fs, str, time::Instant};
use structopt::StructOpt;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Little Kitty =^.^=");

    let opt = Opt::from_args();
    log::debug!("Command line arguments: {:#?}", opt);

    let turi_address: Address = opt
        .turi_address
        .parse()
        .expect("Invalid turi address provided");

    let identity_bytes =
        fs::read_to_string(opt.private_key_file).expect("Could not open private key file.");

    let client = create_client(turi_address.clone(), &identity_bytes);

    match opt.cmd {
        Cmd::Set(cmd) => main_set(client, cmd).await,
        Cmd::Benchmark(cmd) => main_benchmark(identity_bytes, turi_address, cmd).await,
        Cmd::UpdateAccount(cmd) => main_update_account(client, cmd).await,
        Cmd::CreateAccount(cmd) => main_create_account(client, cmd).await,
        Cmd::DeleteAccount(cmd) => main_delete_account(client, cmd).await,
        Cmd::GetValue(cmd) => main_get_value(client, cmd).await,
        Cmd::GetAccount(cmd) => main_get_account(client, cmd).await,
        Cmd::GetBlock(cmd) => main_get_block(client, cmd).await,
        Cmd::CurrentBlockNumber => main_current_block_number(client).await,
    }
}

fn create_client(turi_address: Address, identity: &str) -> Client {
    let identity = identity
        .parse()
        .expect("Cannot read identity. Wrong format?");
    Client::new(turi_address, identity)
}

async fn main_set(mut client: Client, cmd: cmd::Set) {
    let cmd::Set { key, value } = cmd;

    // execute the test client
    match client.send_key_value(key, value).await {
        Err(err) => log::error!("Failed to send transaction: {}", err),
        Ok(()) => log::debug!("Transaction ok!"),
    }
}

async fn main_benchmark(identity: String, turi_address: Address, cmd: cmd::Benchmark) {
    let cmd::Benchmark {
        key,
        transactions,
        size,
        workers,
    } = cmd;

    let mut worker_handles = Vec::new();
    for _ in 0..workers {
        let key = key.clone();
        let address = turi_address.clone();
        let identity = identity.clone();
        worker_handles.push(tokio::spawn(async move {
            let mut client = create_client(address, &identity);
            drop(identity);
            let mut rng = StdRng::from_rng(OsRng {}).unwrap();
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

async fn main_update_account(mut client: Client, cmd: cmd::UpdateAccount) {
    let cmd::UpdateAccount {
        peer_id,
        permission_file,
    } = cmd;

    // TestCLI: 256cdb0197402705f96d39eab7dd3d47a39cb75673a58852d83f666973d80e01
    let peer_id = peer_id.parse().expect("Invalid account id given");

    // Read `Permissions` from the given file.
    let permission_file_content =
        fs::read_to_string(permission_file).expect("Could not read permission file");
    let permissions: Permissions =
        serde_yaml::from_str(&permission_file_content).expect("Invalid permission file content");

    match client.update_account(peer_id, permissions).await {
        Err(err) => log::error!("Failed to send transaction: {}", err),
        Ok(()) => log::debug!("Transaction ok!"),
    }
}

async fn main_create_account(mut client: Client, cmd: cmd::CreateAccount) {
    let cmd::CreateAccount {
        peer_id,
        name,
        permission_file,
    } = cmd;

    let peer_id = peer_id.parse().expect("Invalid account id given.");
    // Read `Permissions` from the given file.
    let permission_file_content =
        fs::read_to_string(permission_file).expect("Could not read permission file.");
    let permissions: Permissions =
        serde_yaml::from_str(&permission_file_content).expect("Invalid permission file content.");

    match client.create_account(peer_id, name, permissions).await {
        Err(err) => log::error!("Failed to send transaction: {}", err),
        Ok(()) => log::debug!("Transaction ok!"),
    }
}

async fn main_delete_account(mut client: Client, cmd: cmd::DeleteAccount) {
    let cmd::DeleteAccount { peer_id } = cmd;
    let peer_id = peer_id.parse().expect("Invalid account id given.");
    match client.delete_account(peer_id).await {
        Err(err) => log::error!("Failed to send transaction: {}", err),
        Ok(()) => log::debug!("Transaction ok!"),
    }
}

async fn main_get_value(mut client: Client, cmd: cmd::GetValue) {
    let cmd::GetValue {
        peer_id,
        filter,
        span,
        end,
        skip,
    } = cmd;

    let query = Query::Range {
        span: span.0,
        end: end.0,
        skip: skip.map(|skip| skip.0),
    };

    match client.query_values(vec![peer_id], filter.0, query).await {
        Ok(values) => {
            if values.is_empty() {
                log::warn!("No values retrieved.");
            }

            for (peer_id, values_of_peer) in values {
                if values_of_peer.is_empty() {
                    log::warn!("No values retrieved for peer {}.", peer_id);
                } else {
                    log::info!("The retrieved values of peer {} are:", peer_id);
                }
                for (key, values_by_key) in values_of_peer {
                    if values_by_key.is_empty() {
                        log::warn!("No values retrieved for key {:?}.", key);
                    } else {
                        log::info!("  Key {:?}:", key);
                    }
                    for (timestamp, (value, client_time, signature)) in values_by_key {
                        log::info!(
                            "    {} (Client Timestamp: {}): {:?}",
                            humantime::format_rfc3339_millis(timestamp),
                            humantime::format_rfc3339_millis(client_time),
                            (value, signature)
                        );
                    }
                }
            }
        }
        Err(err) => log::error!("Failed to retrieve values: {}", err),
    }
}

async fn main_get_account(mut client: Client, cmd: cmd::GetAccount) {
    let cmd::GetAccount { peer_ids } = cmd;

    match client.query_account(peer_ids).await {
        Ok(accounts) => {
            if accounts.is_empty() {
                log::warn!("No accounts retrieved.");
            } else {
                log::info!("The retrieved accounts are:");
            }
            for account in accounts {
                log::info!("{:#?}", account);
            }
        }
        Err(err) => log::error!("Failed to retrieve accounts: {}", err),
    }
}

async fn main_get_block(mut client: Client, cmd: cmd::GetBlock) {
    let cmd::GetBlock { filter } = cmd;

    match client.query_block(filter.0).await {
        Ok(block_vec) => {
            if block_vec.is_empty() {
                log::warn!("No blocks retrieved for the given range.");
            } else {
                log::info!("The retrieved blocks are:");
            }
            for block in block_vec {
                log::info!("{:#?}", block);
            }
        }
        Err(err) => log::error!("Failed to retrieve blocks: {}", err),
    }
}

async fn main_current_block_number(mut client: Client) {
    match client.current_block_number().await {
        Err(err) => log::error!("Failed to retrieve current block number: {}", err),
        Ok(block_number) => log::info!(
            "The current block number is: {:?}. The last committed block number is: {:?}.",
            block_number,
            block_number - 1
        ),
    }
}
