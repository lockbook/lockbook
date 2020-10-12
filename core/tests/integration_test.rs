#![allow(dead_code)]

use lockbook_core::model::account::Account;
use lockbook_core::model::crypto::SignedValue;
use lockbook_core::model::crypto::*;
use lockbook_core::model::state::Config;
use lockbook_core::repo::db_provider::{DbProvider, TempBackedDB};
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::{
    AesImpl, PubKeyCryptoService, RsaImpl, SymmetricCryptoService,
};
use lockbook_core::Db;
use rsa::RSAPublicKey;
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
        keys: RsaImpl::generate_key().unwrap(),
    }
}

pub fn sign(account: &Account) -> SignedValue {
    AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap()
}

pub fn aes_key(encrypting_key: &AesKey, encrypted_key: &AesKey) -> EncryptedValueWithNonce {
    AesImpl::encrypt(
        &encrypting_key,
        &DecryptedValue {
            secret: encrypted_key.key.clone(),
        },
    )
    .unwrap()
}

pub fn aes_str(encrypting_key: &AesKey, encrypted_str: &str) -> EncryptedValueWithNonce {
    AesImpl::encrypt(
        &encrypting_key,
        &DecryptedValue {
            secret: String::from(encrypted_str),
        },
    )
    .unwrap()
}

pub fn aes_decrypt_str(encrypting_key: &AesKey, encrypted_str: &EncryptedValueWithNonce) -> String {
    AesImpl::decrypt(&encrypting_key, &encrypted_str)
        .unwrap()
        .secret
}

pub fn rsa_key(encrypting_key: &RSAPublicKey, encrypted_key: &AesKey) -> EncryptedValue {
    RsaImpl::encrypt(
        encrypting_key,
        &DecryptedValue {
            secret: encrypted_key.key.clone(),
        },
    )
    .unwrap()
}
