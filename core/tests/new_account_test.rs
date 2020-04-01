extern crate lockbook_core;
use lockbook_core::lockbook_api::{new_account, NewAccountError, NewAccountParams};
use lockbook_core::lockbook_api::{create_file, CreateFileError, CreateFileParams};
use lockbook_core::lockbook_api::{change_file_content, ChangeFileContentError, ChangeFileContentParams};
use lockbook_core::lockbook_api::{rename_file, RenameFileError, RenameFileParams};
use lockbook_core::lockbook_api::{move_file, MoveFileError, MoveFileParams};
use lockbook_core::lockbook_api::{delete_file, DeleteFileError, DeleteFileParams};
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
    RenameFileError(RenameFileError),
    MoveFileError(MoveFileError),
    DeleteFileError(DeleteFileError),
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

// TODO - change file content file deleted

#[test]
fn test_rename_file() -> Result<(), TestError> {
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

    rename_file(
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
fn test_rename_file_file_not_found() -> Result<(), TestError> {
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

    match rename_file(
        api_loc(),
        &RenameFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: generate_file_id(),
            new_file_name: "new_file_name".to_string(),
        },
    ) {
        Err(RenameFileError::FileNotFound) => Ok(()),
        Ok(_) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::RenameFileError(e)),
    }
}

// TODO - rename file file deleted

#[test]
fn test_move_file() -> Result<(), TestError> {
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

    move_file(
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
fn test_move_file_file_not_found() -> Result<(), TestError> {
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

    match move_file(
        api_loc(),
        &MoveFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: generate_file_id(),
            new_file_path: "new_file_path".to_string(),
        },
    ) {
        Err(MoveFileError::FileNotFound) => Ok(()),
        Ok(_) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::MoveFileError(e)),
    }
}

// TODO - move file file deleted

#[test]
fn test_move_file_file_path_taken() -> Result<(), TestError> {
    let username = generate_username();
    let file_id_a = generate_file_id();
    let file_id_b = generate_file_id();

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
            file_id: file_id_a.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path_a".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    create_file(
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

    match move_file(
        api_loc(),
        &MoveFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id_b.to_string(),
            new_file_path: "file_path_a".to_string(),
        },
    ) {
        Err(MoveFileError::FilePathTaken) => Ok(()),
        Ok(_) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::MoveFileError(e)),
    }
}

#[test]
fn test_delete_file() -> Result<(), TestError> {
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

    delete_file(
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
fn test_delete_file_file_not_found() -> Result<(), TestError> {
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

    match delete_file(
        api_loc(),
        &DeleteFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: generate_file_id(),
        },
    ) {
        Err(DeleteFileError::FileNotFound) => Ok(()),
        Ok(_) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::DeleteFileError(e)),
    }
}

#[test]
fn test_delete_file_file_deleted() -> Result<(), TestError> {
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

    delete_file(
        api_loc(),
        &DeleteFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
        },
    )?;

    match delete_file(
        api_loc(),
        &DeleteFileParams {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
        },
    ) {
        Err(DeleteFileError::FileDeleted) => Ok(()),
        Ok(_) => Err(TestError::ErrorExpected),
        Err(e) => Err(TestError::DeleteFileError(e)),
    }
}