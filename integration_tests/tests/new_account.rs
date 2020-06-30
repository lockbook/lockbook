#[macro_use]
pub mod utils;

use lockbook_core::client::{Client, ClientImpl, Error};
use lockbook_core::model::account::Account;
use lockbook_core::model::api::{NewAccountError};
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::{PubKeyCryptoService, RsaImpl};
use rsa::{BigUint, RSAPrivateKey};
use utils::{generate_account, generate_username};
use uuid::Uuid;

#[test]
fn new_account() {
    let account = generate_account();

    ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ).expect("failed to make new account");
}

#[test]
fn new_account_duplicate() {
    let account = generate_account();

    ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ).expect("failed to make new account");

    assert_matches!(ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ), Err(Error::Api(NewAccountError::UsernameTaken)));
}

#[test]
fn new_account_case_insensitive_username() {
    let account = generate_account();

    ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ).expect("failed to make new account");

    assert_matches!(ClientImpl::new_account(
        &account.username.to_uppercase(),
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ), Err(Error::Api(NewAccountError::UsernameTaken)));
}

#[test]
fn new_account_invalid_username_special() {
    let account = Account {
        username: String::from("Smail&$@"),
        keys: RsaImpl::generate_key().unwrap(),
    };

    assert_matches!(ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ), Err(Error::Api(NewAccountError::UsernameTaken)));
}

#[test]
fn new_account_invalid_username_chinese() {
    let account = Account {
        username: String::from("漢字"),
        keys: RsaImpl::generate_key().unwrap(),
    };

    assert_matches!(ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ), Err(Error::Api(NewAccountError::UsernameTaken)));
}

#[test]
fn new_account_invalid_username_nonsense() {
    let account = Account {
        username: String::from("øπåß∂ƒ©˙∆˚¬≈ç√∫˜µ"),
        keys: RsaImpl::generate_key().unwrap(),
    };

    assert_matches!(ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ), Err(Error::Api(NewAccountError::UsernameTaken)));
}

#[test]
fn new_account_invalid_username_emoji() {
    let account = Account {
        username: String::from("😀😁😂😃😄"),
        keys: RsaImpl::generate_key().unwrap(),
    };

    assert_matches!(ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ), Err(Error::Api(NewAccountError::UsernameTaken)));
}

#[test]
fn new_account_invalid_username_accents() {
    let account = Account {
        username: String::from("ãÁêì"),
        keys: RsaImpl::generate_key().unwrap(),
    };

    assert_matches!(ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ), Err(Error::Api(NewAccountError::UsernameTaken)));
}

#[test]
fn new_account_invalid_public_key() {
    let account = Account {
        username: generate_username(),
        keys: RSAPrivateKey::from_components(
            BigUint::from_bytes_be(b"a"),
            BigUint::from_bytes_be(b"a"),
            BigUint::from_bytes_be(b"a"),
            vec![
                BigUint::from_bytes_le(&vec![105, 101, 60, 173, 19, 153, 3, 192]),
                BigUint::from_bytes_le(&vec![235, 65, 160, 134, 32, 136, 6, 241]),
            ],
        ),
    };

    assert_matches!(ClientImpl::new_account(
        &account.username,
        &AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ), Err(Error::Api(NewAccountError::InvalidPublicKey)));
}

#[test]
fn new_account_invalid_signature() {
    let account = generate_account();

    assert_matches!(ClientImpl::new_account(
        &account.username,
        "",
        account.keys.to_public_key(),
        Uuid::new_v4(),
    ), Err(Error::Api(NewAccountError::InvalidAuth)));
}
