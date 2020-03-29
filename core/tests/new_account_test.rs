extern crate lockbook_core;
use lockbook_core::lockbook_api::{new_account, NewAccountError, NewAccountParams};
use std::env;
use uuid::Uuid;

fn api_loc() -> String {
    match env::var("LOCKBOOK_API_LOCATION") {
        Ok(s) => s,
        Err(e) => panic!("Could not read environment variable LOCKBOOK_API_LOCATION: {}", e)
    }
}

fn generate_username() -> String {
    Uuid::new_v4().to_string()
}

#[test]
fn test_create_user() -> Result<(), NewAccountError> {
    new_account(
        api_loc(),
        &NewAccountParams {
            username: generate_username(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )
}

#[test]
fn test_create_user_duplicate() -> Result<(), NewAccountError> {
    let username = generate_username();

    new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    match new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    ) {
        Err(NewAccountError::UsernameTaken) => Ok(()),
        Ok(()) => Err(NewAccountError::Unspecified), // todo: better way to translate function success to test error
        err => err,
    }
}
