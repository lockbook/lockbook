extern crate lockbook_core;
use lockbook_core::client;
use lockbook_core::client::{NewAccountError, NewAccountRequest};

#[macro_use]
pub mod utils;
use utils::{api_loc, generate_username, TestError};

fn new_account() -> Result<(), TestError> {
    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: generate_username(),
            auth: "test_auth".to_string(),
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
            auth: "test_auth".to_string(),
            public_key: "test_public_key".to_string(),
        },
    )?;

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
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
