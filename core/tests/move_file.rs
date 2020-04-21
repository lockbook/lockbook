extern crate lockbook_core;

use crate::utils::generate_account;

use lockbook_core::client;
use lockbook_core::client::CreateFileRequest;
use lockbook_core::client::DeleteFileRequest;
use lockbook_core::client::NewAccountRequest;
use lockbook_core::client::{MoveFileError, MoveFileRequest};

#[macro_use]
pub mod utils;
use utils::{api_loc, generate_file_id, generate_username, TestError};
use lockbook_core::service::auth_service::{AuthServiceImpl, AuthService};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;

fn move_file() -> Result<(), TestError> {
    let account = generate_account();
    let file_id = generate_file_id();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::create_file(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    client::move_file(
        api_loc(),
        &MoveFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            file_id: file_id.to_string(),
            new_file_path: "new_file_path".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_move_file() {
    assert_matches!(move_file(), Ok(_));
}

fn move_file_file_not_found() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::move_file(
        api_loc(),
        &MoveFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            file_id: generate_file_id(),
            new_file_path: "new_file_path".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_move_file_file_not_found() {
    assert_matches!(
        move_file_file_not_found(),
        Err(TestError::MoveFileError(MoveFileError::FileNotFound))
    );
}

fn move_file_file_deleted() -> Result<(), TestError> {
    let account = generate_account();
    let file_id = generate_file_id();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::create_file(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    client::delete_file(
        api_loc(),
        &DeleteFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            file_id: file_id.to_string(),
        },
    )?;

    client::move_file(
        api_loc(),
        &MoveFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            file_id: file_id.to_string(),
            new_file_path: "new_file_path".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_move_file_file_deleted() {
    assert_matches!(
        move_file_file_deleted(),
        Err(TestError::MoveFileError(MoveFileError::FileDeleted))
    );
}

fn move_file_file_path_taken() -> Result<(), TestError> {
    let account = generate_account();
    let file_id_a = generate_file_id();
    let file_id_b = generate_file_id();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::create_file(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            file_id: file_id_a.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path_a".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    client::create_file(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            file_id: file_id_b.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path_b".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    client::move_file(
        api_loc(),
        &MoveFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth( &account.keys, &account.username.clone()).unwrap(),
            file_id: file_id_b.to_string(),
            new_file_path: "file_path_a".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_move_file_file_path_taken() {
    assert_matches!(
        move_file_file_path_taken(),
        Err(TestError::MoveFileError(MoveFileError::FilePathTaken))
    );
}
