extern crate lockbook_core;
use lockbook_core::client;
use lockbook_core::client::{NewAccountError, NewAccountRequest};

#[macro_use]
pub mod utils;
use utils::{api_loc, generate_username, TestError};
use lockbook_core::service::auth_service::{AuthServiceImpl, AuthService};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::{RsaImpl, PubKeyCryptoService};

fn new_account() -> Result<(), TestError> {
    let username = generate_username();
    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &RsaImpl::generate_key().unwrap(), &username).unwrap(),
            public_key: "test_public_key".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_new_account() {
    assert_matches!(new_account(), Ok(_));
}

fn new_account_duplicate() -> Result<(), TestError> {
    let username = generate_username();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &RsaImpl::generate_key().unwrap(), &username).unwrap(),
            public_key: "test_public_key".to_string(),
        },
    )?;

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &RsaImpl::generate_key().unwrap(), &username).unwrap(),
            public_key: "test_public_key".to_string(),
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
