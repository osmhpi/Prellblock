//! Module used for Broadcasting Messages between all RPUs.

use crate::{
    peer::{PeerMessage, Sender},
    world_state::WorldStateService,
};
use balise::Request;
use futures::future::join_all;

/// A broadcaster for peer messages.
pub struct Broadcaster {
    world_state: WorldStateService,
}

impl Broadcaster {
    /// Create a new Broadcaster
    ///
    /// `world_state` should be a `WorldState` containing all other RPUs peer addresses.
    #[must_use]
    pub const fn new(world_state: WorldStateService) -> Self {
        Self { world_state }
    }

    /// Broadcast a batch to all known peers (stored in `peer_addresses`).
    #[allow(clippy::future_not_send)]
    pub async fn broadcast<T>(&self, message: &T) -> Result<(), balise::Error>
    where
        T: Request<PeerMessage>,
    {
        // Broadcast transaction to all RPUs.
        let results = join_all(
            self.world_state
                .get()
                .peers
                .iter()
                .map(|(_, peer_address)| {
                    let message = message.clone();
                    let peer_address = *peer_address;
                    tokio::spawn(async move {
                        log::trace!("Sending to {}.", peer_address);
                        let mut sender = Sender::new(peer_address);
                        let result = sender.send_request(message).await;
                        match result {
                            Ok(_) => log::trace!("Sent to {}.", peer_address),
                            Err(_) => log::warn!("Failed to send to {}.", peer_address),
                        }
                        result
                    })
                }),
        )
        .await;

        for result in results {
            match result {
                Err(err) => {
                    log::error!("Error while broadcasting data: {}", err);
                }
                Ok(result) => {
                    // Ignore result
                    result?;
                }
            }
        }
        Ok(())
    }
}
