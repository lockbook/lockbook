use lockbook_core::{
    delete_file, get_file_by_path, Error::UiError, Error::Unexpected as UnexpectedError,
    FileDeleteError, GetFileByPathError,
};

use crate::utils::{exit_with, get_account_or_exit, get_config};
use crate::{FILE_NOT_FOUND, UNEXPECTED_ERROR};

pub fn remove(path: &str) {
    get_account_or_exit();
    let config = get_config();

    let meta = match get_file_by_path(&config, path) {
        Ok(meta) => meta,
        Err(err) => match err {
            UiError(GetFileByPathError::NoFileAtThatPath) => exit_with(
                &format!("No file found with the path {}", path),
                FILE_NOT_FOUND,
            ),
            UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    };

    match delete_file(&config, meta.id) {
        Ok(_) => {}
        Err(err) => match err {
            UiError(FileDeleteError::FileDoesNotExist) => exit_with(
                &format!("Cannot delete '{}', file does not exist.", path),
                FILE_NOT_FOUND,
            ),
            UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }
}
