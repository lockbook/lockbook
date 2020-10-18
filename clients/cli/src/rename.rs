use crate::utils::{exit_with, get_account_or_exit, get_config};
use crate::{
    FILE_NAME_NOT_AVAILABLE, FILE_NOT_FOUND, NAME_CONTAINS_SLASH, NAME_EMPTY, NO_ROOT_OPS,
    UNEXPECTED_ERROR,
};
use lockbook_core::{
    get_file_by_path, rename_file, Error as CoreError, GetFileByPathError, RenameFileError,
};
use std::process::exit;

pub fn rename(path: &str, new_name: &str) {
    get_account_or_exit();

    match get_file_by_path(&get_config(), path) {
        Ok(file_metadata) => match rename_file(&get_config(), file_metadata.id, new_name) {
            Ok(_) => exit(0),
            Err(err) => match err {
                CoreError::UiError(RenameFileError::NewNameEmpty) => {
                    exit_with("New name is empty!", NAME_EMPTY)
                }
                CoreError::UiError(RenameFileError::CannotRenameRoot) => {
                    exit_with("Cannot rename root directory!", NO_ROOT_OPS)
                }
                CoreError::UiError(RenameFileError::NewNameContainsSlash) => {
                    exit_with("New name cannot contain a slash!", NAME_CONTAINS_SLASH)
                }
                CoreError::UiError(RenameFileError::FileNameNotAvailable) => {
                    exit_with("File name not available!", FILE_NAME_NOT_AVAILABLE)
                }
                CoreError::UiError(RenameFileError::FileDoesNotExist) => {
                    exit_with("Unexpected: FileDoesNotExist!", UNEXPECTED_ERROR)
                }
                CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            },
        },
        Err(err) => match err {
            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                exit_with(&format!("No file at {}", path), FILE_NOT_FOUND)
            }
            CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }
}
