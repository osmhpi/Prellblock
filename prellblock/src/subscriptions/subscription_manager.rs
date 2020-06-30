use super::{exporter::ThingsBoard, Exporter, SubscriptionConfig, SubscriptionMeta};
use crate::{block_storage, block_storage::BlockStorage};
use pinxit::PeerId;
use std::{
    boxed::Box,
    collections::{HashMap, HashSet},
    fmt::Debug,
};

/// Manages subscriptions of timeseries.
#[derive(Debug)]
pub struct SubscriptionManager {
    block_storage: BlockStorage,
    /// Every PeerId can have a number of subscriptions, corresponds to a device name.
    subscriptions: HashMap<PeerId, SubscriptionMeta>,
    http_client: reqwest::Client,
    exporters: Vec<Box<dyn Exporter + Send + Sync + 'static>>,
}

impl SubscriptionManager {
    /// This creates a new `SubscriptionManager`.
    pub async fn new(block_storage: BlockStorage) -> Self {
        let mut exporters: Vec<Box<dyn Exporter + Send + Sync>> = Vec::new();
        exporters.push(Box::new(ThingsBoard::new(
            "127.0.0.1:8080".parse().unwrap(),
        )));

        let mut subscriptions: HashMap<PeerId, SubscriptionMeta> = HashMap::new();
        let config = SubscriptionConfig::load();
        for subscription in config.subscription {
            if let Some(meta) = subscriptions.get_mut(&subscription.peer_id) {
                meta.namespaces.insert(subscription.namespace);
            } else {
                let mut subscribed_namespaces = HashSet::new();
                subscribed_namespaces.insert(subscription.namespace);
                subscriptions.insert(
                    subscription.peer_id,
                    SubscriptionMeta {
                        namespaces: subscribed_namespaces,
                        device_name: subscription.device_name,
                        device_type: subscription.device_type,
                    },
                );
            }
        }

        let mut manager = Self {
            block_storage,
            subscriptions,
            http_client: reqwest::Client::new(),
            exporters,
        };

        for exporter in &mut manager.exporters {
            if let Err(err) = exporter.setup_server(&manager.subscriptions).await {
                log::warn!("Error while setting up exporter: {}", err);
            }
        }

        log::info!("SubscriptionManager setup successfully.");
        manager
    }

    /// This will be called on an Apply-`Block` event.
    pub async fn notify_block_update(
        &self,
        data: Vec<(PeerId, String)>,
    ) -> Result<(), block_storage::Error> {
        for (peer_id, namespace) in data {
            if let Some(meta) = self.subscriptions.get(&peer_id) {
                if meta.namespaces.contains(&namespace) {
                    // get transaction from block_storage
                    let transaction = self
                        .block_storage
                        .read_last_transaction(&peer_id, &namespace)?;
                    // post transaction to thingsboard
                    if let Some((_, value)) = transaction.iter().next() {
                        for exporter in &self.exporters {
                            exporter.push(&peer_id, &namespace, &value.0).await;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
