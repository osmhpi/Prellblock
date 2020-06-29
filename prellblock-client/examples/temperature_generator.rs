//! This example demonstrates data generation,
//! using a noise function.
//! The generated data will be sent to an RPU.

use noise::{Fbm, NoiseFn};
use prellblock_client::Client;
use std::time::{Duration, Instant};
use tokio::time::{delay_until, timeout};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let duration = 60000; // in milliseconds = 60s
    let interval = 100; // in milliseconds = 0.1s
    let rpu_address = "127.0.0.1:3131".parse()?;
    // Data will be written into this account.
    let identity = "406ed6170c8672e18707fb7512acf3c9dbfc6e5ad267d9a57b9c486a94d99dcc".parse()?;

    pretty_env_logger::init();
    let mut client = Client::new(rpu_address, identity);

    let res = timeout(
        Duration::from_millis(duration),
        generate_data(&mut client, interval),
    )
    .await;

    if res.is_err() {
        log::info!("Done generating.");
    } else {
        unreachable!();
    }

    Ok(())
}

// This generates data and sends it via a `Client`.
async fn generate_data(client: &mut Client, interval: u64) {
    let start = Instant::now();
    let noise = Fbm::new();

    loop {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(interval);
        let time = Instant::now() - start;
        let time = time.as_secs_f64();
        let mut value = 10_f64.mul_add(noise.get([time, time, time]), 20_f64);
        value = (value * 100.0).round() / 100.0;
        log::trace!("Generated temperature: {}.", value);

        match client
            .send_key_value("temperature".to_string(), value)
            .await
        {
            Err(err) => log::error!("Failed to send transaction: {}", err),
            Ok(()) => log::debug!("Transaction ok!"),
        }
        delay_until(deadline).await;
    }
}
