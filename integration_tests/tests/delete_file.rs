use lockbook_core::client;
use lockbook_core::client::delete_file;
use lockbook_core::model::api::CreateFileRequest;
use lockbook_core::model::api::NewAccountRequest;
use lockbook_core::model::api::{DeleteFileError, DeleteFileRequest};

#[macro_use]
pub mod utils;
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;
use utils::{api_loc, generate_account, generate_file_id, TestError};

fn delete_file() -> Result<(), TestError> {
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

    let version = client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?.current_metadata_and_content_version;

    client::delete_file::send(
        api_loc(),
        &DeleteFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            old_metadata_version: version,
        },
    )?;

    Ok(())
}

#[test]
fn test_delete_file() {
    assert_matches!(delete_file(), Ok(_));
}

fn delete_file_file_not_found() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::delete_file::send(
        api_loc(),
        &DeleteFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: generate_file_id(),
            old_metadata_version: 0,
        },
    )?;

    Ok(())
}

#[test]
fn test_delete_file_file_not_found() {
    assert_matches!(
        delete_file_file_not_found(),
        Err(TestError::DeleteFileError(delete_file::Error::API(
            DeleteFileError::FileNotFound
        )))
    );
}

fn delete_file_file_deleted() -> Result<(), TestError> {
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

    let version = client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?.current_metadata_and_content_version;

    let version = client::delete_file::send(
        api_loc(),
        &DeleteFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            old_metadata_version: version,
        },
    )?.current_metadata_and_content_version;

    client::delete_file::send(
        api_loc(),
        &DeleteFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            old_metadata_version: version,
        },
    )?;

    Ok(())
}

#[test]
fn test_delete_file_file_deleted() {
    assert_matches!(
        delete_file_file_deleted(),
        Err(TestError::DeleteFileError(delete_file::Error::API(
            DeleteFileError::FileDeleted
        )))
    );
}
