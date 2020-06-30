use super::Exporter;
use crate::subscriptions::{error::Error, SubscriptionMeta};
use async_trait::async_trait;
use http::StatusCode;
use pinxit::PeerId;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, net::SocketAddr};

const ENV_THINGSBOARD_USERNAME: &str = "THINGSBOARD_USER_NAME";
const ENV_THINGSBOARD_PASSWORD: &str = "THINGSBOARD_PASSWORD";
const ENV_THINGSBOARD_TENANT_ID: &str = "THINGSBOARD_TENANT_ID";

/// Use `ThingsBoard` for exporting data to a server.
#[derive(Debug)]
pub struct ThingsBoard {
    /// The address to reach your ThingsBoard server.
    host: SocketAddr,
    /// Username to login into ThingsBoard.
    username: String,
    /// Password to login into ThingsBoard.
    password: String,
    /// The token will be received automatically when creating an instance.
    access_token: Option<String>,
    /// The client communicates with the ThingsBoard server.
    http_client: Client,
}

impl ThingsBoard {
    /// Create a new `ThingsBoard`.
    pub fn new(host: SocketAddr) -> Self {
        // Get the environment variables for the thingboard account and password
        let username = if let Ok(username) = env::var(ENV_THINGSBOARD_USERNAME) {
            username
        } else {
            panic!("{} environment variable not set.", ENV_THINGSBOARD_USERNAME);
        };

        let password = if let Ok(password) = env::var(ENV_THINGSBOARD_PASSWORD) {
            password
        } else {
            panic!("{} environment variable not set.", ENV_THINGSBOARD_USERNAME);
        };

        Self {
            host,
            username,
            password,
            access_token: None,
            http_client: Client::new(),
        }
    }

    /// This builds the right url to retrieve login credentials.
    fn login_url(&self) -> String {
        format!("http://{}/api/auth/login", self.host)
    }

    /// This builds the right url to create a new device.
    fn device_url(&self, device_access_token: &str) -> String {
        format!(
            "http://{}/api/device?accessToken={}",
            self.host, device_access_token
        )
    }

    /// This builds the right url to send telemetry data.
    fn telemetry_url(&self, device_access_token: &str) -> String {
        format!(
            "http://{}/api/v1/{}/telemetry",
            self.host, device_access_token
        )
    }

    /// Retrieve an `access_token` for the user.
    async fn login(&mut self) -> Result<(), Error> {
        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct Tokens {
            token: String,
            refresh_token: String,
        }

        let body = serde_json::json!({ "username": self.username, "password": self.password });

        let request = reqwest::Client::new()
            .post(&self.login_url())
            .header("Content-Type", "application/json")
            .body(body.to_string());

        let response = request.send().await?.text().await?;
        let response: Tokens = serde_json::from_str(&response).unwrap();
        self.access_token = Some(response.token);
        Ok(())
    }
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

#[async_trait]
impl Exporter for ThingsBoard {
    /// This creates devices in ThingsBoard for each `PeerId` in the subscriptions.
    async fn setup_server(
        &mut self,
        subscriptions: &HashMap<PeerId, SubscriptionMeta>,
    ) -> Result<(), Error> {
        // login into thingsboard and set the access_token
        if self.access_token.is_none() {
            self.login().await?;
        }

        // FIXME: Maybe auto via API?
        #[allow(clippy::single_match_else)]
        let tenant_id = match env::var(ENV_THINGSBOARD_TENANT_ID) {
            Ok(tenant_id) => tenant_id,
            Err(_) => {
                return Err(Error::ThingsboardTenantIdNotSet);
            }
        };

        for (peer_id, meta) in subscriptions {
            // create a new device via POST
            let url = self.device_url(&peer_id.to_string());

            let body = build_thingsboard_device(
                meta.device_name.clone(),
                meta.device_type.clone(),
                tenant_id.clone(),
            );

            // unwrap should be safe because we logged in beforehand
            let access_token = self.access_token.as_ref().unwrap();
            let request = self
                .http_client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("X-Authorization", format!("Bearer:{}", access_token))
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

    async fn push(&self, peer_id: &PeerId, namespace: &str, value: &[u8]) {
        let device_access_token = peer_id.to_string();
        let url = self.telemetry_url(&device_access_token);
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
