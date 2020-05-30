use lockbook_core::client;
use lockbook_core::client::create_file;
use lockbook_core::model::api::CreateFileError;
use lockbook_core::model::api::CreateFileRequest;
use lockbook_core::model::api::NewAccountRequest;

#[macro_use]
pub mod utils;
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;
use utils::{api_loc, generate_account, generate_file_id, TestError};

fn create_file() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: generate_file_id(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_create_file() {
    assert_matches!(create_file(), Ok(_));
}

fn create_file_case_insensitive_username() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.to_uppercase(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: generate_file_id(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_create_file_case_insensitive_username() {
    assert_matches!(create_file_case_insensitive_username(), Ok(_));
}

fn create_file_duplicate_file_id() -> Result<(), TestError> {
    let account = generate_account();
    let file_id = generate_file_id();

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path_2".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_create_file_duplicate_file_id() {
    assert_matches!(
        create_file_duplicate_file_id(),
        Err(TestError::CreateFileError(create_file::Error::API(
            CreateFileError::FileIdTaken
        )))
    );
}

fn create_file_duplicate_file_path() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: generate_file_id(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: generate_file_id(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_create_file_duplicate_file_path() {
    assert_matches!(
        create_file_duplicate_file_path(),
        Err(TestError::CreateFileError(create_file::Error::API(
            CreateFileError::FilePathTaken
        )))
    );
}

fn create_file_alphanumeric_username() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: "Smail#@%!".to_string(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: generate_file_id(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_create_file_alphanumeric_username() {
    assert_matches!(
        create_file_alphanumeric_username(),
        Err(TestError::CreateFileError(create_file::Error::API(
            CreateFileError::InvalidUsername
        )))
    );
}
