//! Module used for Broadcasting Messages between all RPUs.

use crate::{
    peer::{PeerMessage, Sender},
    BoxError,
};
use balise::Request;
use futures::future::join_all;
use std::net::SocketAddr;

/// A broadcaster for peer messages.
pub struct Broadcaster {
    peer_addresses: Vec<SocketAddr>,
}

impl Broadcaster {
    /// Create a new Broadcaster
    ///
    /// `peer_addresses` should be a Vector of all other RPUs peer addresses.
    #[must_use]
    pub fn new(peer_addresses: Vec<SocketAddr>) -> Self {
        Self { peer_addresses }
    }

    /// Broadcast a batch to all known peers (stored in `peer_addresses`).
    pub async fn broadcast<T>(&self, message: &T) -> Result<(), BoxError>
    where
        T: Request<PeerMessage>,
    {
        // Broadcast transaction to all RPUs.
        let results = join_all(self.peer_addresses.iter().map(|peer_address| {
            let message = message.clone();
            async move {
                let mut sender = Sender::new(*peer_address);
                sender.send_request(message).await
            }
        }))
        .await;

        for result in results {
            // Ignore result
            let _ = result?;
        }
        Ok(())
    }
}
