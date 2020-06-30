//! The module communicating with a thingsboard.

use pinxit::PeerId;
use serde::Deserialize;
use std::{collections::HashSet, fs};

mod error;
mod exporter;
mod subscription_manager;

use exporter::Exporter;
pub use subscription_manager::SubscriptionManager;

#[derive(Debug, Clone, Deserialize)]
struct SubscriptionConfig {
    pub subscription: Vec<Subscription>,
}

/// The representation of a subscription in the configuration file.
#[derive(Debug, Clone, Deserialize)]
struct Subscription {
    pub peer_id: PeerId,
    pub namespace: String,
    pub device_name: String,
    pub device_type: String,
}

impl SubscriptionConfig {
    pub fn load() -> Self {
        let config_data = fs::read_to_string("./config/subscription_config.toml").unwrap();
        toml::from_str(&config_data).unwrap()
    }
}

/// Internal representation for subscriptions of a specific `PeerId`.
#[derive(Debug)]
struct SubscriptionMeta {
    /// All subscribed namespaces.
    namespaces: HashSet<String>,
    /// The name of the device in ThingsBoard (only when freshly created).
    device_name: String,
    /// The device's group label.
    device_type: String,
}
