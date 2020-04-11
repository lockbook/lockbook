extern crate lockbook_core;
use lockbook_core::lockbook_api;
use lockbook_core::lockbook_api::{ChangeFileContentError, ChangeFileContentParams};
use lockbook_core::lockbook_api::{CreateFileError, CreateFileParams};
use lockbook_core::lockbook_api::{DeleteFileError, DeleteFileParams};
use lockbook_core::lockbook_api::{FileMetadata, GetUpdatesError, GetUpdatesParams};
use lockbook_core::lockbook_api::{MoveFileError, MoveFileParams};
use lockbook_core::lockbook_api::{NewAccountError, NewAccountParams};
use lockbook_core::lockbook_api::{RenameFileError, RenameFileParams};
use std::env;
use uuid::Uuid;

fn api_loc() -> String {
    match env::var("LOCKBOOK_API_LOCATION") {
        Ok(s) => s,
        Err(e) => panic!(
            "Could not read environment variable LOCKBOOK_API_LOCATION: {}",
            e
        ),
    }
}

fn generate_username() -> String {
    Uuid::new_v4().to_string()
}

fn generate_file_id() -> String {
    Uuid::new_v4().to_string()
}

macro_rules! assert_matches(
    ($actual:expr, $expected:pat) => {
        // Only compute actual once
        let actual_value = $actual;
        match actual_value {
            $expected => {},
            _ => panic!("assertion failed: {:?} did not match expectation", actual_value)
        }
    }
);

#[derive(Debug)]
enum TestError {
    NewAccountError(NewAccountError),
    CreateFileError(CreateFileError),
    ChangeFileContentError(ChangeFileContentError),
    RenameFileError(RenameFileError),
    MoveFileError(MoveFileError),
    DeleteFileError(DeleteFileError),
    GetUpdatesError(GetUpdatesError),
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

impl From<RenameFileError> for TestError {
    fn from(e: RenameFileError) -> TestError {
        TestError::RenameFileError(e)
    }
}

impl From<MoveFileError> for TestError {
    fn from(e: MoveFileError) -> TestError {
        TestError::MoveFileError(e)
    }
}

impl From<DeleteFileError> for TestError {
    fn from(e: DeleteFileError) -> TestError {
        TestError::DeleteFileError(e)
    }
}

impl From<GetUpdatesError> for TestError {
    fn from(e: GetUpdatesError) -> TestError {
        TestError::GetUpdatesError(e)
    }
}

fn new_account() -> Result<(), TestError> {
    lockbook_api::new_account(
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
fn test_new_account() {
    assert_matches!(new_account(), Ok(_));
}

fn new_account_duplicate() -> Result<(), TestError> {
    let username = generate_username();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    Ok(())
}

#[test]
fn test_new_account_duplicate() {
    assert_matches!(
        new_account_duplicate(),
        Err(TestError::NewAccountError(NewAccountError::UsernameTaken))
    );
}

fn create_file() -> Result<(), TestError> {
    let username = generate_username();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
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
fn test_create_file() {
    assert_matches!(create_file(), Ok(_));
}

fn create_file_duplicate_file_id() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
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

    lockbook_api::create_file(
        api_loc(),
        &CreateFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
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
        Err(TestError::CreateFileError(CreateFileError::FileIdTaken))
    );
}

fn create_file_duplicate_file_path() -> Result<(), TestError> {
    let username = generate_username();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
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

    lockbook_api::create_file(
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
fn test_create_file_duplicate_file_path() {
    assert_matches!(
        create_file_duplicate_file_path(),
        Err(TestError::CreateFileError(CreateFileError::FilePathTaken))
    );
}

fn change_file_content() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    let old_file_version = lockbook_api::create_file(
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

    lockbook_api::change_file_content(
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
fn test_change_file_content() {
    assert_matches!(change_file_content(), Ok(_));
}

fn change_file_content_file_not_found() -> Result<(), TestError> {
    let username = generate_username();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::change_file_content(
        api_loc(),
        &ChangeFileContentParams {
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

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
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

    lockbook_api::change_file_content(
        api_loc(),
        &ChangeFileContentParams {
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

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    let old_file_version = lockbook_api::create_file(
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

    lockbook_api::delete_file(
        api_loc(),
        &DeleteFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
        },
    )?;

    lockbook_api::change_file_content(
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
fn test_change_file_content_file_deleted() {
    assert_matches!(
        change_file_content_file_deleted(),
        Err(TestError::ChangeFileContentError(
            ChangeFileContentError::FileDeleted
        ))
    );
}

fn rename_file() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
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

    lockbook_api::rename_file(
        api_loc(),
        &RenameFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
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
    let username = generate_username();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::rename_file(
        api_loc(),
        &RenameFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
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
        Err(TestError::RenameFileError(RenameFileError::FileNotFound))
    );
}

fn rename_file_file_deleted() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
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

    lockbook_api::delete_file(
        api_loc(),
        &DeleteFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
        },
    )?;

    lockbook_api::rename_file(
        api_loc(),
        &RenameFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
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
        Err(TestError::RenameFileError(RenameFileError::FileDeleted))
    );
}

fn move_file() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
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

    lockbook_api::move_file(
        api_loc(),
        &MoveFileParams {
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

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::move_file(
        api_loc(),
        &MoveFileParams {
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

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
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

    lockbook_api::delete_file(
        api_loc(),
        &DeleteFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
        },
    )?;

    lockbook_api::move_file(
        api_loc(),
        &MoveFileParams {
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

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
        api_loc(),
        &CreateFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id_a.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path_a".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    lockbook_api::create_file(
        api_loc(),
        &CreateFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id_b.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path_b".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    lockbook_api::move_file(
        api_loc(),
        &MoveFileParams {
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

fn delete_file() -> Result<(), TestError> {
    let username = generate_username();
    let file_id = generate_file_id();

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
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

    lockbook_api::delete_file(
        api_loc(),
        &DeleteFileParams {
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

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::delete_file(
        api_loc(),
        &DeleteFileParams {
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

    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    lockbook_api::create_file(
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

    lockbook_api::delete_file(
        api_loc(),
        &DeleteFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
        },
    )?;

    lockbook_api::delete_file(
        api_loc(),
        &DeleteFileParams {
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

fn get_updates(username: String, file_id: String) -> Result<(Vec<FileMetadata>, u64), TestError> {
    lockbook_api::new_account(
        api_loc(),
        &NewAccountParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    let file_version = lockbook_api::create_file(
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

    let updates_metadata = lockbook_api::get_updates(
        api_loc(),
        &GetUpdatesParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            since_version: 0,
        },
    )?;

    Ok((updates_metadata, file_version))
}

#[test]
fn test_get_updates() {
    let username = generate_username();
    let file_id = generate_file_id();

    let updates_metadata_and_file_version = get_updates(username.to_string(), file_id.to_string());
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
