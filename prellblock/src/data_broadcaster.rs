//! Module used for Broadcasting Messages between all RPUs.

use crate::{
    peer::{PeerMessage, Sender},
    thread_group::ThreadGroup,
    BoxError,
};
use balise::Request;
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
    pub fn broadcast<T>(&self, message: &T) -> Result<(), BoxError>
    where
        T: Request<PeerMessage>,
    {
        let mut thread_group = ThreadGroup::new();

        // Broadcast transaction to all RPUs.
        for &peer_address in &self.peer_addresses {
            let message = message.clone();
            //let message = message::ExecuteBatch(batch);
            thread_group.spawn(format!("Sender ({})", peer_address), move || {
                let mut sender = Sender::new(peer_address);
                sender.send_request(message)
            });
        }
        //join threads
        thread_group.join_and_log();
        Ok(())
    }
}
