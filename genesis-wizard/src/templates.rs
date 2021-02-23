// mod account;
use prellblock_client_api::account::Permissions;
use serde::{Deserialize};

#[derive(Deserialize)]
pub struct AccountTypeTemplate {
    #[serde(rename = "type")]
    pub account_type: String,
    pub turi_address: String,
    pub peer_address: String
}

#[derive(Deserialize)]
pub struct AccountTemplate {
    pub name: String,
    // #[serde(rename = "type")]
    // pub account_type: AccountTypeTemplate
    pub permissions: Permissions
}
