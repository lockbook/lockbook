#[macro_use]
pub mod utils;

use lockbook_core::client::{Client, ClientImpl};
use lockbook_core::model::account::Account;
use lockbook_core::model::api::{NewAccountError};
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::{PubKeyCryptoService, RsaImpl};
use rsa::{BigUint, RSAPrivateKey, RsaPublicKey};
use utils::{generate_account, generate_username, TestError};
use uuid::Uuid;

fn new_account() -> Result<(), TestError> {
    let account = generate_account();

    ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    )?;

    Ok(())
}

#[test]
fn test_new_account() {
    assert_matches!(new_account(), Ok(_));
}

fn new_account_duplicate() -> Result<(), TestError> {
    let account = generate_account();

    ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    )?;

    ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
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

fn new_account_case_insensitive_username() -> Result<(), TestError> {
    let account = generate_account();

    ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    )?;

    ClientImpl::new_account(
        &account.username.to_uppercase(),
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    )?;

    Ok(())
}

#[test]
fn test_new_account_case_insensitive_username() {
    assert_matches!(
        new_account_case_insensitive_username(),
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

    ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
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

fn new_account_invalid_public_key() -> Result<(), TestError> {
    let account = Account {
        username: generate_username(),
        keys: RsaImpl::generate_key().unwrap(),
    };

    let fake_private_key = RSAPrivateKey::from_components(
        BigUint::from_bytes_be(b"a"),
        BigUint::from_bytes_be(b"a"),
        BigUint::from_bytes_be(b"a"),
        vec![
            BigUint::from_bytes_le(&vec![105, 101, 60, 173, 19, 153, 3, 192]),
            BigUint::from_bytes_le(&vec![235, 65, 160, 134, 32, 136, 6, 241]),
        ],
    );

    ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        fake_private_key.to_public_key(),
        Uuid::new_v4(),
    )?;

    Ok(())
}

#[test]
fn test_new_account_invalid_public_key() {
    assert_matches!(
        new_account_invalid_public_key(),
        Err(TestError::NewAccountError(new_account::Error::API(
            NewAccountError::InvalidPublicKey
        )))
    );
}

fn new_account_invalid_auth() -> Result<(), TestError> {
    let account = generate_account();

    ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        RsaPublicKey{},
        Uuid::new_v4(),
    )?;

    Ok(())
}

#[test]
fn test_new_account_invalid_auth() {
    assert_matches!(
        new_account_invalid_auth(),
        Err(TestError::NewAccountError(new_account::Error::API(
            NewAccountError::InvalidAuth
        )))
    );
}
