use crate::utils::{exit_with, get_account_or_exit, get_config};
use crate::{FILE_NOT_FOUND, UNEXPECTED_ERROR};
use lockbook_core::{get_file_by_path, read_document, Error as CoreError, GetFileByPathError};

pub fn print(file_name: &str) {
    get_account_or_exit();

    let file_metadata = match get_file_by_path(&get_config(), &file_name) {
        Ok(fm) => fm,
        Err(err) => match err {
            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                exit_with("File not found", FILE_NOT_FOUND)
            }
            CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    };

    match read_document(&get_config(), file_metadata.id) {
        Ok(content) => print!("{}", content.secret),
        Err(error) => panic!("Unexpected error: {:?}", error),
    };
}
