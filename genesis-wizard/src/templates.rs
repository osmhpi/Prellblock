// mod account;
use prellblock_client_api::account::Permissions;
use serde::{Deserialize};

#[derive(Deserialize)]
pub struct AccountTemplate {
    pub name: String,
    pub permissions: Permissions
}
