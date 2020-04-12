extern crate lockbook_core;
use lockbook_core::lockbook_api::{CreateFileRequest, NewAccountClientImpl, CreateFileClientImpl, MoveFileClientImpl, DeleteFileClientImpl};
use lockbook_core::lockbook_api::DeleteFileRequest;
use lockbook_core::lockbook_api::NewAccountRequest;
use lockbook_core::lockbook_api::{MoveFileError, MoveFileRequest};

#[macro_use]
pub mod utils;
use utils::{api_loc, generate_file_id, generate_username, TestError};
use lockbook_core::lockbook_api::new_account::NewAccountClient;
use lockbook_core::lockbook_api::create_file::CreateFileClient;
use lockbook_core::lockbook_api::move_file::MoveFileClient;
use lockbook_core::lockbook_api::delete_file::DeleteFileClient;

fn move_file() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    NewAccountClientImpl::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    CreateFileClientImpl::create_file(
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

    MoveFileClientImpl::move_file(
        api_loc(),
        &MoveFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
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

    MoveFileClientImpl::move_file(
        api_loc(),
        &MoveFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
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
    let username = generate_username();
    let file_id = generate_file_id();

    NewAccountClientImpl::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    CreateFileClientImpl::create_file(
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

    DeleteFileClientImpl::delete_file(
        api_loc(),
        &DeleteFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
        },
    )?;

    MoveFileClientImpl::move_file(
        api_loc(),
        &MoveFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
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
    let username = generate_username();
    let file_id_a = generate_file_id();
    let file_id_b = generate_file_id();

    NewAccountClientImpl::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    CreateFileClientImpl::create_file(
        api_loc(),
        &CreateFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id_a.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path_a".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    CreateFileClientImpl::create_file(
        api_loc(),
        &CreateFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id_b.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path_b".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    MoveFileClientImpl::move_file(
        api_loc(),
        &MoveFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
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
