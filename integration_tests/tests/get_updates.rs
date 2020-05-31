use crate::utils::generate_account;

use lockbook_core::client;
use lockbook_core::client::get_updates;
use lockbook_core::model::api::{
    CreateFileRequest, FileMetadata, GetUpdatesError, GetUpdatesRequest, NewAccountRequest,
};

#[macro_use]
pub mod utils;

use lockbook_core::model::account::Account;
use lockbook_core::service::auth_service::{AuthService, AuthServiceImpl};
use lockbook_core::service::clock_service::ClockImpl;
use lockbook_core::service::crypto_service::RsaImpl;
use utils::{api_loc, generate_file_id, TestError};

fn get_updates(account: &Account, file_id: String) -> Result<(Vec<FileMetadata>, u64), TestError> {
    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    let file_version = client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?
    .current_version;

    let updates_metadata = client::get_updates::send(
        api_loc(),
        &GetUpdatesRequest {
            username: account.username.clone(),
            auth: "test_auth".to_string(),
            since_version: 0,
        },
    )?
    .file_metadata;

    Ok((updates_metadata, file_version))
}

#[test]
fn test_get_updates() {
    let account = generate_account();
    let file_id = generate_file_id();

    let updates_metadata_and_file_version = get_updates(&account, file_id.to_string());
    assert_matches!(&updates_metadata_and_file_version, &Ok(_));
    let (updates_metadata, file_version) = updates_metadata_and_file_version.unwrap();
    assert_eq!(
        updates_metadata[..],
        [FileMetadata {
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content_version: file_version,
            file_metadata_version: file_version,
            deleted: false,
        }][..]
    );
}

fn get_updates_case_insensitive_username(
    account: &Account,
    file_id: String,
) -> Result<(Vec<FileMetadata>, u64), TestError> {
    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    let file_version = client::create_file::send(
        api_loc(),
        &CreateFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?
    .current_version;

    let updates_metadata = client::get_updates::send(
        api_loc(),
        &GetUpdatesRequest {
            username: account.username.to_uppercase(),
            auth: "test_auth".to_string(),
            since_version: 0,
        },
    )?
    .file_metadata;

    Ok((updates_metadata, file_version))
}

#[test]
fn test_get_updates_case_insensitive_username() {
    let account = generate_account();
    let file_id = generate_file_id();

    let updates_metadata_and_file_version =
        get_updates_case_insensitive_username(&account, file_id.to_string());
    assert_matches!(&updates_metadata_and_file_version, &Ok(_));
    let (updates_metadata, file_version) = updates_metadata_and_file_version.unwrap();
    assert_eq!(
        updates_metadata[..],
        [FileMetadata {
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content_version: file_version,
            file_metadata_version: file_version,
            deleted: false,
        }][..]
    );
}

fn get_updates_alphanumeric_username(username: String) -> Result<(), TestError> {
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
    )?
    .current_version;

    client::get_updates::send(
        api_loc(),
        &GetUpdatesRequest {
            username: username,
            auth: "test_auth".to_string(),
            since_version: 0,
        },
    )?
    .file_metadata;

    Ok(())
}

#[test]
fn test_get_updates_alphanumeric_username() {
    assert_matches!(
        get_updates_alphanumeric_username("Smail&$@".to_string()),
        Err(TestError::GetUpdatesError(get_updates::Error::API(
            GetUpdatesError::InvalidUsername
        )))
    );
    assert_matches!(
        get_updates_alphanumeric_username("漢字".to_string()),
        Err(TestError::GetUpdatesError(get_updates::Error::API(
            GetUpdatesError::InvalidUsername
        )))
    );
    assert_matches!(
        get_updates_alphanumeric_username("øπåß∂ƒ©˙∆˚¬≈ç√∫˜µ".to_string()),
        Err(TestError::GetUpdatesError(get_updates::Error::API(
            GetUpdatesError::InvalidUsername
        )))
    );
    assert_matches!(
        get_updates_alphanumeric_username("😀😁😂😃😄".to_string()),
        Err(TestError::GetUpdatesError(get_updates::Error::API(
            GetUpdatesError::InvalidUsername
        )))
    );
    assert_matches!(
        get_updates_alphanumeric_username("ãÁêì".to_string()),
        Err(TestError::GetUpdatesError(get_updates::Error::API(
            GetUpdatesError::InvalidUsername
        )))
    );
}
