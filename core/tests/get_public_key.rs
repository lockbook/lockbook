extern crate lockbook_core;
extern crate serde_json;

use crate::utils::{api_loc, generate_account, TestError};

#[macro_use]
pub mod utils;
use lockbook_core::client;
use lockbook_core::client::{GetPublicKeyRequest, NewAccountRequest, GetPublicKeyError};
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;
use rsa::RSAPrivateKey;
use serde_json::to_string;

fn get_public_key(username: String, keys: RSAPrivateKey) -> Result<String, TestError> {
    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&keys, &username.clone())
                .unwrap(),
            public_key: serde_json::to_string(&keys.to_public_key()).unwrap(),
        },
    )?;

    let retrieved_key = client::get_public_key(
        api_loc(),
        &GetPublicKeyRequest {
            username: username.clone(),
        },
    )?;

    Ok(retrieved_key)
}

#[test]
fn test_get_public_key() {
    let account = generate_account();

    let retrieved_key = get_public_key(account.username.clone(), account.keys.clone()).unwrap();

    let true_key = serde_json::to_string(&account.keys.to_public_key()).unwrap();
    assert_matches!(retrieved_key, true_key); // turn one into another
}

fn get_public_key_invalid() -> Result<(), TestError> {
    let account = generate_account();

    client::get_public_key(
        api_loc(),
        &GetPublicKeyRequest {
            username: account.username.clone(),
        },
    )?;

    Ok(())
}

#[test]
fn test_get_public_key_invalid() {
    assert_matches!(
        get_public_key_invalid(),
        Err(TestError::GetPublicKeyError(GetPublicKeyError::UsernameNotFound))
    );
}
