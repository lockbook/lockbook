extern crate lockbook_core;
use lockbook_core::client;
use lockbook_core::client::CreateFileRequest;
use lockbook_core::client::NewAccountRequest;
use lockbook_core::client::{DeleteFileError, DeleteFileRequest};

#[macro_use]
pub mod utils;
use utils::{api_loc, generate_file_id, generate_username, TestError};

fn delete_file() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            public_key: "test_public_key".to_string(),
        },
    )?;

    client::create_file(
        api_loc(),
        &CreateFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    client::delete_file(
        api_loc(),
        &DeleteFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_delete_file() {
    assert_matches!(delete_file(), Ok(_));
}

fn delete_file_file_not_found() -> Result<(), TestError> {
    let username = generate_username();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            public_key: "test_public_key".to_string(),
        },
    )?;

    client::delete_file(
        api_loc(),
        &DeleteFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: generate_file_id(),
        },
    )?;

    Ok(())
}

#[test]
fn test_delete_file_file_not_found() {
    assert_matches!(
        delete_file_file_not_found(),
        Err(TestError::DeleteFileError(DeleteFileError::FileNotFound))
    );
}

fn delete_file_file_deleted() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            public_key: "test_public_key".to_string(),
        },
    )?;

    client::create_file(
        api_loc(),
        &CreateFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    client::delete_file(
        api_loc(),
        &DeleteFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
        },
    )?;

    client::delete_file(
        api_loc(),
        &DeleteFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_delete_file_file_deleted() {
    assert_matches!(
        delete_file_file_deleted(),
        Err(TestError::DeleteFileError(DeleteFileError::FileDeleted))
    );
}
