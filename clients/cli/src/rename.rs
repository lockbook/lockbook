use crate::utils::{exit_success, get_account_or_exit, get_config};
use crate::{err_unexpected, exitlb};
use lockbook_core::{
    get_file_by_path, rename_file, Error as CoreError, GetFileByPathError, RenameFileError,
};

pub fn rename(path: &str, new_name: &str) {
    get_account_or_exit();

    match get_file_by_path(&get_config(), path) {
        Ok(file_metadata) => match rename_file(&get_config(), file_metadata.id, new_name) {
            Ok(_) => exit_success(""),
            Err(err) => match err {
                CoreError::UiError(RenameFileError::NewNameEmpty) => {
                    exitlb!(FileNameEmpty, "New name is empty!")
                }
                CoreError::UiError(RenameFileError::CannotRenameRoot) => {
                    exitlb!(NoRootOps("rename".to_string()))
                }
                CoreError::UiError(RenameFileError::NewNameContainsSlash) => {
                    exitlb!(FileNameHasSlash, "New name cannot contain a slash!")
                }
                CoreError::UiError(RenameFileError::FileNameNotAvailable) => {
                    exitlb!(FileNameNotAvailable, "File name not available!")
                }
                CoreError::UiError(RenameFileError::FileDoesNotExist) => {
                    err_unexpected!("FileDoesNotExist!").exit()
                }
                CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
            },
        },
        Err(err) => match err {
            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                exitlb!(FileNotFound, "No file at {}", path)
            }
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
        },
    }
}
