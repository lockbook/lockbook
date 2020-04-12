extern crate lockbook_core;
use lockbook_core::lockbook_api::{NewAccountError, NewAccountRequest, NewAccountClientImpl};

#[macro_use]
pub mod utils;
use utils::{api_loc, generate_username, TestError};
use lockbook_core::lockbook_api::new_account::NewAccountClient;

fn new_account() -> Result<(), TestError> {
    NewAccountClientImpl::new_account(
        api_loc(),
        &NewAccountRequest {
            username: generate_username(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
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

    NewAccountClientImpl::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    NewAccountClientImpl::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
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
