extern crate lockbook_core;
use lockbook_core::client;
use lockbook_core::client::CreateFileRequest;
use lockbook_core::client::DeleteFileRequest;
use lockbook_core::client::NewAccountRequest;
use lockbook_core::client::{ChangeFileContentError, ChangeFileContentRequest};

#[macro_use]
pub mod utils;
use utils::{api_loc, generate_file_id, generate_username, TestError};

fn change_file_content() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    let old_file_version = client::create_file(
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

    client::change_file_content(
        api_loc(),
        &ChangeFileContentRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
            old_file_version: old_file_version,
            new_file_content: "new_file_content".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_change_file_content() {
    assert_matches!(change_file_content(), Ok(_));
}

fn change_file_content_file_not_found() -> Result<(), TestError> {
    let username = generate_username();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    client::change_file_content(
        api_loc(),
        &ChangeFileContentRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: generate_file_id(),
            old_file_version: 0,
            new_file_content: "new_file_content".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_change_file_content_file_not_found() {
    assert_matches!(
        change_file_content_file_not_found(),
        Err(TestError::ChangeFileContentError(
            ChangeFileContentError::FileNotFound
        ))
    );
}

fn change_file_content_edit_conflict() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
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

    client::change_file_content(
        api_loc(),
        &ChangeFileContentRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
            old_file_version: 0,
            new_file_content: "new_file_content".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_change_file_content_edit_conflict() {
    assert_matches!(
        change_file_content_edit_conflict(),
        Err(TestError::ChangeFileContentError(ChangeFileContentError::EditConflict(_)))
    );
}

fn change_file_content_file_deleted() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    client::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    let old_file_version = client::create_file(
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

    client::change_file_content(
        api_loc(),
        &ChangeFileContentRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
            old_file_version: old_file_version,
            new_file_content: "new_file_content".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_change_file_content_file_deleted() {
    assert_matches!(
        change_file_content_file_deleted(),
        Err(TestError::ChangeFileContentError(
            ChangeFileContentError::FileDeleted
        ))
    );
}
