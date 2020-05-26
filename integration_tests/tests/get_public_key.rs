extern crate lockbook_core;
extern crate serde_json;

use crate::utils::{api_loc, generate_account, TestError};

#[macro_use]
pub mod utils;
use lockbook_core::client;
use lockbook_core::client::get_public_key;
use lockbook_core::model::api::{GetPublicKeyError, GetPublicKeyRequest, NewAccountRequest};
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;

fn get_public_key() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::get_public_key::send(
        api_loc(),
        &GetPublicKeyRequest {
            username: account.username.clone(),
        },
    )?;

    Ok(())
}

#[test]
fn test_get_public_key() {
    assert_matches!(get_public_key(), Ok(_));
}

fn get_public_key_invalid() -> Result<(), TestError> {
    let account = generate_account();

    client::get_public_key::send(
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
        Err(TestError::GetPublicKeyError(get_public_key::Error::API(
            GetPublicKeyError::UserNotFound
        )))
    );
}
