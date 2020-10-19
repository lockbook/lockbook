#![allow(dead_code)]

use lockbook_core::model::account::Account;
use lockbook_core::model::crypto::*;
use lockbook_core::model::state::Config;
use lockbook_core::repo::db_provider::{DbProvider, TempBackedDB};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::{
    AESImpl, PubKeyCryptoService, RSAImpl, SymmetricCryptoService,
};
use lockbook_core::Db;
use rsa::RSAPublicKey;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::env;
use uuid::Uuid;

#[macro_export]
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

pub fn test_db() -> Db {
    let config = Config {
        writeable_path: "ignored".to_string(),
    };
    TempBackedDB::connect_to_db(&config).unwrap()
}

pub fn test_config() -> Config {
    Config {
        writeable_path: format!("/tmp/{}", Uuid::new_v4().to_string()),
    }
}

pub fn random_username() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn random_filename() -> String {
    Uuid::new_v4()
        .to_string()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn generate_account() -> Account {
    Account {
        username: random_username(),
        api_url: env::var("API_URL").expect("API_URL must be defined!"),
        private_key: RSAImpl::<ClockImpl>::generate_key().unwrap(),
    }
}

pub fn aes_encrypt<T: Serialize + DeserializeOwned>(
    key: &AESKey,
    to_encrypt: &T,
) -> AESEncrypted<T> {
    AESImpl::encrypt(key, to_encrypt).unwrap()
}

pub fn aes_decrypt<T: Serialize + DeserializeOwned>(
    key: &AESKey,
    to_decrypt: &AESEncrypted<T>,
) -> T {
    AESImpl::decrypt(&key, &to_decrypt).unwrap()
}

pub fn rsa_encrypt<T: Serialize + DeserializeOwned>(
    key: &RSAPublicKey,
    to_encrypt: &T,
) -> RSAEncrypted<T> {
    RSAImpl::<ClockImpl>::encrypt(key, to_encrypt).unwrap()
}
