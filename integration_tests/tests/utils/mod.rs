use std::env;
use uuid::Uuid;
use lockbook_core::model::account::Account;
use lockbook_core::service::crypto_service::{PubKeyCryptoService, RsaImpl};

pub fn api_loc() -> String {
    format!(
        "http://{}:{}",
        env_or_panic("SERVER_HOST"),
        env_or_panic("SERVER_PORT")
    )
}

fn env_or_panic(var_name: &str) -> String {
    env::var(var_name).expect(&format!("Missing environment variable {}", var_name))
}

pub fn generate_account() -> Account {
    Account {
        username: generate_username(),
        keys: RsaImpl::generate_key().unwrap(),
    }
}

pub fn generate_username() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

macro_rules! assert_matches (
    ($actual:expr, $expected:pat) => {
        // Only compute actual once
        let actual_value = $actual;
        match actual_value {
            $expected => {},
            _ => panic!("assertion failed: {:?} did not match expectation", actual_value)
        }
    }
);
