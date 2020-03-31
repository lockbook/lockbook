extern crate lockbook_core;
use lockbook_core::lockbook_api::{new_account, NewAccountError, NewAccountParams};
use lockbook_core::lockbook_api::{create_file, CreateFileError, CreateFileParams};
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

#[derive(Debug)]
enum TestError {
    ErrorExpected,
    NewAccountError(NewAccountError),
    CreateFileError(CreateFileError),
}

impl From<NewAccountError> for TestError {
    fn from(e: NewAccountError) -> TestError {
        TestError::NewAccountError(e)
    }
}

impl From<CreateFileError> for TestError {
    fn from(e: CreateFileError) -> TestError {
        TestError::CreateFileError(e)
    }
}

#[test]
fn test_create_user() -> Result<(), TestError> {
    new_account(
        api_loc(),
        &NewAccountParams {
            username: generate_username(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_create_user_duplicate() -> Result<(), TestError> {
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
        Ok(()) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::NewAccountError(e)),
    }
}

#[test]
fn test_create_file() -> Result<(), TestError> {
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

    create_file(
        api_loc(),
        &CreateFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: "file_id".to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_create_file_duplicate_file_id() -> Result<(), TestError> {
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

    create_file(
        api_loc(),
        &CreateFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: "file_id".to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    match create_file(
        api_loc(),
        &CreateFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: "file_id".to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path_2".to_string(),
            file_content: "file_content".to_string(),
        },
    ) {
        Err(CreateFileError::FileIdTaken) => Ok(()),
        Ok(()) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::CreateFileError(e)),
    }
}