use super::{error::Error, subscriptions::SubscriptionConfig};
use crate::{block_storage, block_storage::BlockStorage};
use http::StatusCode;
use pinxit::PeerId;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    env,
    time::Duration,
};
use tokio::time::delay_until;

const ENV_THINGSBOARD_USERNAME: &str = "THINGSBOARD_USER_NAME";
const ENV_THINGSBOARD_PASSWORD: &str = "THINGSBOARD_PASSWORD";
const ENV_THINGSBOARD_TENANT_ID: &str = "THINGSBOARD_TENANT_ID";

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

/// Manages subscriptions of timeseries.
#[derive(Debug)]
pub struct SubscriptionManager {
    block_storage: BlockStorage,
    /// Every PeerId can have a number of subscriptions, corresponds to a device name.
    subscriptions: HashMap<PeerId, SubscriptionMeta>,
    http_client: reqwest::Client,
    user_token: String,
}

impl SubscriptionManager {
    /// This creates a new `SubscriptionManager`.
    pub async fn new(block_storage: BlockStorage) -> Self {
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

        let user_token = loop {
            match Self::user_token().await {
                Ok(user_token) => {
                    break user_token;
                }
                Err(err) => {
                    log::warn!("Error while retrieving the thingsboard user token: {}", err);
                    delay_until(tokio::time::Instant::now() + Duration::from_secs(1)).await;
                }
            }
        };

        let manager = Self {
            block_storage,
            subscriptions,
            http_client: reqwest::Client::new(),
            user_token,
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

        log::info!("SubscriptionManager setup successfully.");
        manager
    }

    /// This creates devices in thingsboard for each accessToken in the subscriptions.
    pub async fn setup_devices(&self) -> Result<(), Error> {
        let client = reqwest::Client::new();

        #[allow(clippy::single_match_else)]
        let tenant_id = match env::var(ENV_THINGSBOARD_TENANT_ID) {
            Ok(tenant_id) => tenant_id,
            Err(_) => {
                return Err(Error::ThingsboardTenantIdNotSet);
            }
        };

        for (peer_id, meta) in &self.subscriptions {
            // create a new device via POST
            let url = device_url(&peer_id.to_string());

            let body = build_thingsboard_device(
                meta.device_name.clone(),
                meta.device_type.clone(),
                tenant_id.clone(),
            );

            let request = client
                .post(&url)
                .header("Content-Type", "application/json")
                .header(
                    "X-Authorization",
                    format!("Bearer:{}", self.user_token).to_string(),
                )
                .body(body.to_string());

            let response = request.send().await?;

            log::trace!(
                "Setup ThingsBoard device {}: {}",
                meta.device_name,
                response.text().await?
            );
        }
        Ok(())
    }

    /// This will retreive the token needed to authenticate device creation query.
    async fn user_token() -> Result<String, Error> {
        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct Tokens {
            token: String,
            refresh_token: String,
        }

        // Get the environment variables for the thingboard account and password
        let thingsboard_username = if let Ok(username) = env::var(ENV_THINGSBOARD_USERNAME) {
            username
        } else {
            return Err(Error::ThingsboardUserNameNotSet);
        };

        let thingsboard_password = if let Ok(password) = env::var(ENV_THINGSBOARD_PASSWORD) {
            password
        } else {
            return Err(Error::ThingsboardPasswordNotSet);
        };

        let body = serde_json::json!({ "username": thingsboard_username, "password": thingsboard_password });

        let url = login_url();

        let request = reqwest::Client::new()
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body.to_string());

        let response = request.send().await?.text().await?;
        let response: Tokens = serde_json::from_str(&response).unwrap();
        Ok(response.token)
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
                        self.post_value(&value.0, &namespace, &peer_id.to_string())
                            .await;
                    }
                }
            }
        }
        Ok(())
    }

    async fn post_value(&self, value: &[u8], namespace: &str, access_token: &str) {
        let url = telemetry_url(access_token);
        // FIXME: Use CBOR or so to automatically convert the data to JSON.
        let value: f64 = postcard::from_bytes(value).unwrap();
        let key_value_json = format!("{{{}:{}}}", namespace, value);
        log::trace!("Sending POST w/ json body: {:#?}", key_value_json);
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
                    log::trace!("Unknown statuscode: {:?}", res.status());
                }
            },
            Err(err) => {
                log::error!("Error sending to ThingsBoard: {}", err);
            }
        }
    }
}

/// This builds the right url to send telemetry data.
fn telemetry_url(access_token: &str) -> String {
    let host = "localhost";
    let port = "8080";
    format!("http://{}:{}/api/v1/{}/telemetry", host, port, access_token)
}

/// This builds the right url to retrieve login credentials.
fn login_url() -> String {
    "http://localhost:8080/api/auth/login".to_string()
}

/// This builds the right url to create a new device.
fn device_url(access_token: &str) -> String {
    format!(
        "http://localhost:8080/api/device?accessToken={}",
        access_token
    )
}

/// This creates a thingsboard device json configuration.
fn build_thingsboard_device(device_name: String, device_type: String, tenant_id: String) -> String {
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Tenant {
        entity_type: String,
        id: String,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Device {
        name: String,
        tenant_id: Tenant,
        entity_type: String,
        r#type: String,
    }

    let device = Device {
        name: device_name,
        tenant_id: Tenant {
            entity_type: "TENANT".to_string(),
            id: tenant_id,
        },
        entity_type: "DEVICE".to_string(),
        r#type: device_type,
    };

    serde_json::json!(device).to_string()
}
