//! Module used for Broadcasting Messages between all RPUs.

use crate::peer::{PeerMessage, Sender};
use balise::Request;
use std::net::SocketAddr;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A broadcaster for peer messages.
///
/// Example (coming soon)
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

    /// Broadcast a message to all known peers (stored in `peer_addresses`).
    pub fn broadcast<T>(&self, message: &T) -> Result<(), BoxError>
    where
        T: Request<PeerMessage>,
    {
        let mut thread_join_handles = Vec::new();
        // Broadcast transaction to all RPUs.
        for &peer_address in &self.peer_addresses {
            let message = message.clone();
            thread_join_handles.push((
                format!("Sender ({})", peer_address),
                std::thread::spawn(move || {
                    let mut sender = Sender::new(peer_address);
                    match sender.send_request(message) {
                        Ok(_) => log::debug!("Successfully sent message to peer {}", peer_address),
                        Err(err) => {
                            log::error!("Failed sending message to peer {}: {}", peer_address, err)
                        }
                    }
                }),
            ));
        }
        //join threads
        for (name, join_handle) in thread_join_handles {
            match join_handle.join() {
                Err(err) => log::error!("Error occurred waiting for {}: {:?}", name, err),
                Ok(()) => log::info!("Ended {}.", name),
            };
        }
        Ok(())
    }
}
