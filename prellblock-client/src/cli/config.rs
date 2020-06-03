use serde::Deserialize;
use std::{fs, net::SocketAddr};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub rpu: Vec<Rpu>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rpu {
    pub name: String,
    pub peer_id: String,
    pub peer_address: SocketAddr,
    pub turi_address: SocketAddr,
}

impl Config {
    pub fn load() -> Self {
        let config_data = fs::read_to_string("./config/config.toml").unwrap();
        toml::from_str(&config_data).unwrap()
    }
}
