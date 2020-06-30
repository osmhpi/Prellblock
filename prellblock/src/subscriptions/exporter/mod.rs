use super::SubscriptionMeta;
use crate::subscriptions::error::Error;
use async_trait::async_trait;
use pinxit::PeerId;
use std::{collections::HashMap, fmt::Debug};

mod thingsboard;
pub use thingsboard::ThingsBoard;

/// A Exporter can communicate with an external service to store and export data there.
#[async_trait]
pub(super) trait Exporter: Debug {
    /// Execute all necessary steps to initialize things on the server
    /// (e.g. create client accounts).
    async fn setup_server(
        &mut self,
        subscriptions: &HashMap<PeerId, SubscriptionMeta>,
    ) -> Result<(), Error>;

    /// Push a new value to the server.
    async fn push(&self, peer_id: &PeerId, namespace: &str, value: &[u8]);
}
