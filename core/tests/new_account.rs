extern crate lockbook_core;
use lockbook_core::client;
use lockbook_core::client::{NewAccountError, NewAccountRequest};

#[macro_use]
pub mod utils;
use utils::{api_loc, generate_username, TestError};
use lockbook_core::service::auth_service::{AuthServiceImpl, AuthService};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::{RsaImpl, PubKeyCryptoService};
use crate::utils::generate_account;

fn new_account() -> Result<(), TestError> {
    let account = generate_account();
    let key = RsaImpl::generate_key().unwrap();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            public_key: serde_json::to_string(&key.to_public_key()).unwrap(),
        },
    )?;

    Ok(())
}

#[test]
fn test_new_account() {
    assert_matches!(new_account(), Ok(_));
}

fn new_account_duplicate() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    Ok(())
}

#[test]
fn test_new_account_duplicate() {
    assert_matches!(
        new_account_duplicate(),
        Err(TestError::NewAccountError(NewAccountError::UsernameTaken))
    );
}
