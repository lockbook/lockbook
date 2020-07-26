use crate::utils::{exit_with, exit_with_no_account, get_config};
use crate::{FILE_NAME_NOT_AVAILABLE, FILE_NOT_FOUND, NAME_CONTAINS_SLASH, UNEXPECTED_ERROR};
use lockbook_core::{
    get_account, get_file_by_path, rename_file, GetAccountError, GetFileByPathError,
    RenameFileError,
};
use std::process::exit;

pub fn rename(path: &str, new_name: &str) {
    match get_account(&get_config()) {
        Ok(_) => {}
        Err(err) => match err {
            GetAccountError::NoAccount => exit_with_no_account(),
            GetAccountError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }

    match get_file_by_path(&get_config(), path) {
        Ok(file_metadata) => match rename_file(&get_config(), file_metadata.id, new_name) {
            Ok(_) => exit(0),
            Err(err) => match err {
                RenameFileError::NewNameContainsSlash => {
                    exit_with("New name cannot contain a slash!", NAME_CONTAINS_SLASH)
                }
                RenameFileError::FileNameNotAvailable => {
                    exit_with("File name not available!", FILE_NAME_NOT_AVAILABLE)
                }
                RenameFileError::FileDoesNotExist => {
                    exit_with("Unexpected: FileDoesNotExist!", UNEXPECTED_ERROR)
                }
                RenameFileError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            },
        },
        Err(err) => match err {
            GetFileByPathError::NoFileAtThatPath => {
                exit_with(&format!("No file at {}", path), FILE_NOT_FOUND)
            }
            GetFileByPathError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }
}
