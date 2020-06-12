use super::subscriptions::SubscriptionConfig;
use crate::{block_storage, block_storage::BlockStorage, consensus::Error};
use http::StatusCode;
use pinxit::PeerId;
use serde::Deserialize;
use std::{collections::HashMap, env, time::Duration};
use tokio::time::delay_until;

const ENV_THINGSBOARD_USERNAME: &str = "THINGSBOARD_USER_NAME";
const ENV_THINGSBOARD_PASSWORD: &str = "THINGSBOARD_PASSWORD";

/// Manages subscriptions of timeseries.
#[derive(Debug)]
pub struct SubscriptionManager {
    block_storage: BlockStorage,
    subscriptions: HashMap<PeerId, (HashMap<String, ()>, String)>,
    http_client: reqwest::Client,
    user_token: Option<String>,
}

impl SubscriptionManager {
    /// This creates a new `SubscriptionManager`.
    #[must_use]
    pub async fn new(block_storage: BlockStorage) -> Self {
        let mut subscriptions: HashMap<PeerId, (HashMap<String, ()>, _)> = HashMap::new();
        let config = SubscriptionConfig::load();
        for subscription in config.subscription {
            if let Some((peer_map, _)) = subscriptions.get_mut(&subscription.peer_id) {
                peer_map.insert(subscription.namespace, ());
            } else {
                let mut peer_map = HashMap::new();
                peer_map.insert(subscription.namespace, ());
                subscriptions.insert(subscription.peer_id, (peer_map, subscription.access_token));
            }
        }

        let manager = Self {
            block_storage,
            subscriptions,
            http_client: reqwest::Client::new(),
            user_token: None,
        };

        loop {
            match manager.setup_devices().await {
                Ok(()) => {
                    break;
                }
                Err(err) => {
                    log::warn!("Error while setting up thingsboard devices: {}", err);
                }
            }
            delay_until(tokio::time::Instant::now() + Duration::from_secs(1)).await;
        }

        return manager;
    }

    /// This creates devices in thingsboard for each accessToken in the subscriptions.
    pub async fn setup_devices(&self) -> Result<(), Error> {
        self.user_token().await?;
        for (_, access_token) in self.subscriptions.iter() {
            // create a new device via POST
        }
        Ok(())
    }

    async fn user_token(&self) -> Result<String, Error> {
        #[derive(Deserialize, Debug)]
        struct Tokens {
            token: String,
            refresh_token: String,
        }

        // Get the environment variables for the thingboard account and password
        let thingsboard_username = match env::var(ENV_THINGSBOARD_USERNAME) {
            Ok(username) => username,
            Err(_) => {
                return Err(Error::ThingsboardUserNameNotSet);
            }
        };
        let thingsboard_password = match env::var(ENV_THINGSBOARD_PASSWORD) {
            Ok(password) => password,
            Err(_) => {
                return Err(Error::ThingsboardPasswordNotSet);
            }
        };

        let body = format!(
            "{{username:{}, password: {} }}",
            thingsboard_username, thingsboard_password
        );
        let url = "http://localhost:8080/api/auth/login";

        let request = self
            .http_client
            .post(url)
            .header("Content-Type", "application/json")
            .body(body);

        let response = request.send().await?.text().await?;
        let response: Tokens = serde_json::from_str(&response).unwrap();
        Ok(response.token)
    }

    /// This will be called on an Applay-`Block` event.
    pub async fn notify_block_update(
        &self,
        data: Vec<(PeerId, String)>,
    ) -> Result<(), block_storage::Error> {
        for (peer_id, namespace) in data {
            if let Some((peer_map, access_token)) = self.subscriptions.get(&peer_id) {
                if peer_map.contains_key(&namespace) {
                    // get transaction from block_storage
                    let transaction = self.block_storage.read_transaction(&peer_id, &namespace)?;
                    // post transaction to thingsboard
                    if let Some((_, value)) = transaction.iter().next() {
                        self.post_value(&value.0, &namespace, access_token).await;
                    }
                }
            }
        }
        Ok(())
    }
    async fn post_value(&self, value: &[u8], namespace: &str, access_token: &str) {
        let url = thingsboard_url(access_token);
        let value: f64 = postcard::from_bytes(value).unwrap();
        let key_value_json = format!("{{{}:{}}}", namespace, value);
        log::trace!("Sending POST w/ json body: {}", key_value_json);
        let body = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(key_value_json);
        //send request
        let res = body.send().await;
        match res {
            Ok(res) => match res.status() {
                StatusCode::OK => log::trace!("Send POST successfully."),
                StatusCode::BAD_REQUEST => log::warn!("BAD_REQUEST response from {}.", url),
                _ => {
                    log::trace!("Statuscode: {:?}", res.status());
                }
            },
            Err(err) => {
                log::error!("{}", err);
            }
        }
    }
}

fn thingsboard_url(access_token: &str) -> String {
    let host = "localhost";
    let port = "8080";
    format!("http://{}:{}/api/v1/{}/telemetry", host, port, access_token)
}
