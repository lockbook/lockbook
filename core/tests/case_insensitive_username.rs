extern crate lockbook_core;
extern crate serde_json;

use crate::utils::{api_loc, TestError};

use lockbook_core::client;
use lockbook_core::client::{GetPublicKeyRequest, NewAccountRequest};
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::{RsaImpl, PubKeyCryptoService};
use lockbook_core::model::account::Account;

#[macro_use]
pub mod utils;

fn check_case_insensitive() -> Result<(), TestError> {
    let account = Account {
        username: "SMAIL".to_string(),
        keys: RsaImpl::generate_key().unwrap()
    };

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(
                &account.keys,
                &account.username.clone(),
            )
                .unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::get_public_key(
        api_loc(),
        &GetPublicKeyRequest {
            username: account.username.to_lowercase(),
        },
    )?;

    Ok(())
}

#[test]
fn test_case_insensitive_username() {
    assert_matches!(check_case_insensitive(), Ok(_));
}
