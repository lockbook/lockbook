use lockbook_core::client;
use lockbook_core::client::rename_file;
use lockbook_core::model::api::CreateFileRequest;
use lockbook_core::model::api::DeleteFileRequest;
use lockbook_core::model::api::NewAccountRequest;
use lockbook_core::model::api::{RenameFileError, RenameFileRequest};

#[macro_use]
pub mod utils;
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
    )?
    .current_metadata_and_content_version;

    client::rename_file::send(
        api_loc(),
        &RenameFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            old_metadata_version: version,
            new_file_name: "new_file_name".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_rename_file() {
    assert_matches!(rename_file(), Ok(_));
}

fn rename_file_case_insensitive_username() -> Result<(), TestError> {
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

    client::rename_file::send(
        api_loc(),
        &RenameFileRequest {
            username: account.username.to_uppercase(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            old_metadata_version: version,
            new_file_name: "new_file_name".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_rename_file_case_insensitive_username() {
    assert_matches!(
        rename_file_case_insensitive_username(),
        Err(TestError::RenameFileError(rename_file::Error::API(
            RenameFileError::InvalidUsername
        )))
    );
}

fn rename_file_file_not_found() -> Result<(), TestError> {
    let account = generate_account();

    client::new_account::send(
        api_loc(),
        &NewAccountRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            public_key: serde_json::to_string(&account.keys.to_public_key()).unwrap(),
        },
    )?;

    client::rename_file::send(
        api_loc(),
        &RenameFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: generate_file_id(),
            old_metadata_version: 0,
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
    )?
    .current_metadata_and_content_version;

    let version = client::delete_file::send(
        api_loc(),
        &DeleteFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            old_metadata_version: version,
        },
    )?
    .current_metadata_and_content_version;

    client::rename_file::send(
        api_loc(),
        &RenameFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            old_metadata_version: version,
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

fn rename_file_edit_conflict() -> Result<(), TestError> {
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
    )?
    .current_metadata_and_content_version;

    client::rename_file::send(
        api_loc(),
        &RenameFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            old_metadata_version: version,
            new_file_name: "new_file_name".to_string(),
        },
    )?;

    client::rename_file::send(
        api_loc(),
        &RenameFileRequest {
            username: account.username.clone(),
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            old_metadata_version: version,
            new_file_name: "new_file_name".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_rename_file_edit_conflict() {
    assert_matches!(
        rename_file_edit_conflict(),
        Err(TestError::RenameFileError(rename_file::Error::API(
            RenameFileError::EditConflict
        )))
    );
}

fn rename_file_alphanumeric_username(username: String) -> Result<(), TestError> {
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

    client::rename_file::send(
        api_loc(),
        &RenameFileRequest {
            username: username,
            auth: AuthServiceImpl::<ClockImpl, RsaImpl>::generate_auth(&account).unwrap(),
            file_id: file_id.to_string(),
            old_metadata_version: version,
            new_file_name: "new_file_name".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_rename_file_alphanumeric_username() {
    assert_matches!(
        rename_file_alphanumeric_username("Smail&$@".to_string()),
        Err(TestError::RenameFileError(rename_file::Error::API(
            RenameFileError::InvalidUsername
        )))
    );
    assert_matches!(
        rename_file_alphanumeric_username("漢字".to_string()),
        Err(TestError::RenameFileError(rename_file::Error::API(
            RenameFileError::InvalidUsername
        )))
    );
    assert_matches!(
        rename_file_alphanumeric_username("øπåß∂ƒ©˙∆˚¬≈ç√∫˜µ".to_string()),
        Err(TestError::RenameFileError(rename_file::Error::API(
            RenameFileError::InvalidUsername
        )))
    );
    assert_matches!(
        rename_file_alphanumeric_username("😀😁😂😃😄".to_string()),
        Err(TestError::RenameFileError(rename_file::Error::API(
            RenameFileError::InvalidUsername
        )))
    );
    assert_matches!(
        rename_file_alphanumeric_username("ãÁêì".to_string()),
        Err(TestError::RenameFileError(rename_file::Error::API(
            RenameFileError::InvalidUsername
        )))
    );
}
