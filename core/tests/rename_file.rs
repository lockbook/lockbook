extern crate lockbook_core;

use lockbook_core::client;
use lockbook_core::client::rename_file;
use lockbook_core::model::api::CreateFileRequest;
use lockbook_core::model::api::DeleteFileRequest;
use lockbook_core::model::api::NewAccountRequest;
use lockbook_core::model::api::{RenameFileError, RenameFileRequest};

#[macro_use]
mod utils;
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;
use utils::{api_loc, generate_account, generate_file_id, TestError};

fn rename_file() -> Result<(), TestError> {
    let account = generate_account();
    let file_id = generate_file_id();

    client::new_account::send(
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

    client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(
                &account.keys,
                &account.username.clone(),
            )
            .unwrap(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    client::rename_file::send(
        api_loc(),
        &RenameFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(
                &account.keys,
                &account.username.clone(),
            )
            .unwrap(),
            file_id: file_id.to_string(),
            new_file_name: "new_file_name".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_rename_file() {
    assert_matches!(rename_file(), Ok(_));
}

fn rename_file_file_not_found() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account::send(
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

    client::rename_file::send(
        api_loc(),
        &RenameFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(
                &account.keys,
                &account.username.clone(),
            )
            .unwrap(),
            file_id: generate_file_id(),
            new_file_name: "new_file_name".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_rename_file_file_not_found() {
    assert_matches!(
        rename_file_file_not_found(),
        Err(TestError::RenameFileError(rename_file::Error::API(
            RenameFileError::FileNotFound
        )))
    );
}

fn rename_file_file_deleted() -> Result<(), TestError> {
    let account = generate_account();
    let file_id = generate_file_id();

    client::new_account::send(
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

    client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(
                &account.keys,
                &account.username.clone(),
            )
            .unwrap(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    client::delete_file::send(
        api_loc(),
        &DeleteFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(
                &account.keys,
                &account.username.clone(),
            )
            .unwrap(),
            file_id: file_id.to_string(),
        },
    )?;

    client::rename_file::send(
        api_loc(),
        &RenameFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(
                &account.keys,
                &account.username.clone(),
            )
            .unwrap(),
            file_id: file_id.to_string(),
            new_file_name: "new_file_name".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_rename_file_file_deleted() {
    assert_matches!(
        rename_file_file_deleted(),
        Err(TestError::RenameFileError(rename_file::Error::API(
            RenameFileError::FileDeleted
        )))
    );
}
