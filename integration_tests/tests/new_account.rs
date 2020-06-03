use lockbook_core::client;
use lockbook_core::client::new_account;
use lockbook_core::model::account::Account;
use lockbook_core::model::api::{NewAccountError, NewAccountRequest};

#[macro_use]
pub mod utils;
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::{PubKeyCryptoService, RsaImpl};
use utils::{api_loc, generate_account, TestError};

fn new_account() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
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

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    Ok(())
}

#[test]
fn test_new_account_duplicate() {
    assert_matches!(
        new_account_duplicate(),
        Err(TestError::NewAccountError(new_account::Error::API(
            NewAccountError::UsernameTaken
        )))
    );
}

fn new_account_case_sensitive_username() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.to_uppercase(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    Ok(())
}

#[test]
fn test_new_account_case_sensitive_username() {
    assert_matches!(
        new_account_case_sensitive_username(),
        Err(TestError::NewAccountError(new_account::Error::API(
            NewAccountError::InvalidUsername
        )))
    );
}

fn new_account_alphanumeric_username(username: String) -> Result<(), TestError> {
    let account = Account {
        username: username,
        keys: RsaImpl::generate_key().unwrap(),
    };

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    Ok(())
}

#[test]
fn test_new_account_alphanumeric_username() {
    assert_matches!(
        new_account_alphanumeric_username("Smail&$@".to_string()),
        Err(TestError::NewAccountError(new_account::Error::API(
            NewAccountError::InvalidUsername
        )))
    );
    assert_matches!(
        new_account_alphanumeric_username("漢字".to_string()),
        Err(TestError::NewAccountError(new_account::Error::API(
            NewAccountError::InvalidUsername
        )))
    );
    assert_matches!(
        new_account_alphanumeric_username("øπåß∂ƒ©˙∆˚¬≈ç√∫˜µ".to_string()),
        Err(TestError::NewAccountError(new_account::Error::API(
            NewAccountError::InvalidUsername
        )))
    );
    assert_matches!(
        new_account_alphanumeric_username("😀😁😂😃😄".to_string()),
        Err(TestError::NewAccountError(new_account::Error::API(
            NewAccountError::InvalidUsername
        )))
    );
    assert_matches!(
        new_account_alphanumeric_username("ãÁêì".to_string()),
        Err(TestError::NewAccountError(new_account::Error::API(
            NewAccountError::InvalidUsername
        )))
    );
}
