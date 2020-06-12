use pinxit::PeerId;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionConfig {
    pub subscription: Vec<Subscription>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Subscription {
    pub peer_id: PeerId,
    pub access_token: String,
    pub namespace: String,
}

impl SubscriptionConfig {
    pub fn load() -> Self {
        let config_data = fs::read_to_string("./config/subscription_config.toml").unwrap();
        toml::from_str(&config_data).unwrap()
    }
}
