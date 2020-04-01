extern crate lockbook_core;
use lockbook_core::lockbook_api::{new_account, NewAccountError, NewAccountParams};
use lockbook_core::lockbook_api::{create_file, CreateFileError, CreateFileParams};
use lockbook_core::lockbook_api::{change_file_content, ChangeFileContentError, ChangeFileContentParams};
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

fn generate_file_id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug)]
enum TestError {
    ErrorExpected,
    NewAccountError(NewAccountError),
    CreateFileError(CreateFileError),
    ChangeFileContentError(ChangeFileContentError),
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

impl From<ChangeFileContentError> for TestError {
    fn from(e: ChangeFileContentError) -> TestError {
        TestError::ChangeFileContentError(e)
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
            file_id: generate_file_id(),
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
    let file_id = generate_file_id();

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
            file_id: file_id.to_string(),
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
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path_2".to_string(),
            file_content: "file_content".to_string(),
        },
    ) {
        Err(CreateFileError::FileIdTaken) => Ok(()),
        Ok(_) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::CreateFileError(e)),
    }
}

#[test]
fn test_create_file_duplicate_file_path() -> Result<(), TestError> {
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
            file_id: generate_file_id(),
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
            file_id: generate_file_id(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    ) {
        Err(CreateFileError::FilePathTaken) => Ok(()),
        Ok(_) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::CreateFileError(e)),
    }
}

#[test]
fn test_change_file_content() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    let old_file_version = create_file(
        api_loc(),
        &CreateFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    change_file_content(
        api_loc(),
        &ChangeFileContentParams {
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
fn test_change_file_content_file_not_found() -> Result<(), TestError> {
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

    match change_file_content(
        api_loc(),
        &ChangeFileContentParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: generate_file_id(),
            old_file_version: 0,
            new_file_content: "new_file_content".to_string(),
        },
    ) {
        Err(ChangeFileContentError::FileNotFound) => Ok(()),
        Ok(_) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::ChangeFileContentError(e)),
    }
}

#[test]
fn test_change_file_content_edit_conflict() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

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
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    match change_file_content(
        api_loc(),
        &ChangeFileContentParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
            old_file_version: 0,
            new_file_content: "new_file_content".to_string(),
        },
    ) {
        Err(ChangeFileContentError::EditConflict(_)) => Ok(()),
        Ok(_) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::ChangeFileContentError(e)),
    }
}