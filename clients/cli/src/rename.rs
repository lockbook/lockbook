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
                CoreError::UiError(err) => match err {
                    RenameFileError::NewNameEmpty => exitlb!(FileNameEmpty),
                    RenameFileError::CannotRenameRoot => exitlb!(NoRootOps("rename".to_string())),
                    RenameFileError::NewNameContainsSlash => {
                        exitlb!(FileNameHasSlash(new_name.to_string()))
                    }
                    RenameFileError::FileNameNotAvailable => {
                        exitlb!(FileNameNotAvailable(new_name.to_string()))
                    }
                    RenameFileError::FileDoesNotExist => {
                        err_unexpected!("FileDoesNotExist!").exit()
                    }
                },
                CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
            },
        },
        Err(err) => match err {
            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                exitlb!(FileNotFound(path.to_string()))
            }
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
        },
    }
}
