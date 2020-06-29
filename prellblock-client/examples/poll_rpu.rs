//! This example shows how to use the Client's API for reading transactions
//! by polling a value periodically.
//! While running this example, generate some data (e.g. with the `temperature_generator` example).

use prellblock_client::{Client, Filter, PeerId, Query};
use std::time::Duration;
use tokio::time::delay_until;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let polling_interval = 10000; // in milliseconds = 10s
    let peer_id_to_read: PeerId =
        "256cdb0197402705f96d39eab7dd3d47a39cb75673a58852d83f666973d80e01".parse()?;
    // The value to read periodically.
    let namespace = "temperature".to_string();
    // The own account (having read access to the given namespace in `peer_id_to_read`).
    let identity = "03d738c972f37a6fd9b33278ac0c50236e45637bcd5aeee82d8323655257d256".parse()?;
    let rpu_address = "127.0.0.1:3131".parse()?;

    pretty_env_logger::init();

    let mut client = Client::new(rpu_address, identity);
    log::info!("Polling in an interval of {} ms.", polling_interval);
    loop {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(polling_interval);
        // Only read the last value.
        let query = Query::Range {
            span: 1.into(),
            end: 0.into(),
            skip: None,
        };
        match client
            .query_values(
                vec![peer_id_to_read.clone()],
                Filter::Exact(namespace.clone()), // have a look at the `Filter` docs for more examples
                query,
            )
            .await
        {
            Ok(values) => {
                println!("{:#?}", values);
            }
            Err(err) => {
                log::warn!("Error while querying data: {}", err);
            }
        }
        delay_until(deadline).await;
    }
}
