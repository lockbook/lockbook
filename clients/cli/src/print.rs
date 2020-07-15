use crate::utils::{exit_with, exit_with_no_account, get_config};
use crate::{UNEXPECTED_ERROR, FILE_NOT_FOUND};
use lockbook_core::{get_account, GetAccountError, read_document, get_file_by_path, GetFileByPathError};

pub fn print(file_name: &str) {
    match get_account(&get_config()) {
        Ok(_) => {}
        Err(err) => match err {
            GetAccountError::NoAccount => exit_with_no_account(),
            GetAccountError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }

    let file_metadata = match get_file_by_path(&get_config(), &file_name) {
        Ok(fm) => fm,
        Err(err) => match err {
            GetFileByPathError::NoFileAtThatPath => exit_with("File not found", FILE_NOT_FOUND),
            GetFileByPathError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    };

    match read_document(&get_config(), file_metadata.id) {
        Ok(content) => print!("{}", content.secret),
        Err(error) => panic!("Unexpected error: {:?}", error),
    };
}
