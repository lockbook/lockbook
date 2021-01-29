use crate::utils::{get_account_or_exit, get_config};
use crate::{err_unexpected, exitlb};
use lockbook_core::{get_file_by_path, read_document, Error as CoreError, GetFileByPathError};

pub fn print(file_name: &str) {
    get_account_or_exit();

    let file_metadata = match get_file_by_path(&get_config(), &file_name) {
        Ok(fm) => fm,
        Err(err) => match err {
            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                exitlb!(FileNotFound, "File not found")
            }
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
        },
    };

    match read_document(&get_config(), file_metadata.id) {
        Ok(content) => print!("{}", String::from_utf8_lossy(&content)),
        Err(error) => panic!("unexpected error: {:?}", error),
    };
}
