#![allow(dead_code)]

use lockbook_core::model::state::Config;
use lockbook_core::storage::db_provider::Backend;
use lockbook_core::DefaultBackend;
use lockbook_crypto::clock_service::ClockImpl;
use lockbook_crypto::crypto_service::{PubKeyCryptoService, RSAImpl};
use lockbook_models::account::Account;

use std::env;
use uuid::Uuid;

pub fn test_db() -> <DefaultBackend as Backend>::Db {
    <DefaultBackend as Backend>::connect_to_db(&test_config()).unwrap()
}

pub fn test_config() -> Config {
    Config {
        writeable_path: format!("/tmp/{}", Uuid::new_v4().to_string()),
    }
}

pub fn api_url() -> String {
    env::var("API_URL").expect("API_URL must be defined!")
}

pub fn random_uuid() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
