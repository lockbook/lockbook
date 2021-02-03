use crate::error::CliResult;
use crate::utils::{get_account_or_exit, get_config};
use crate::{err, err_unexpected};
use lockbook_core::{
    get_file_by_path, rename_file, Error as CoreError, GetFileByPathError, RenameFileError,
};

pub fn rename(path: &str, new_name: &str) -> CliResult {
    get_account_or_exit();

    let file_metadata = get_file_by_path(&get_config(), path).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(path.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    rename_file(&get_config(), file_metadata.id, new_name).map_err(|err| match err {
        CoreError::UiError(err) => match err {
            RenameFileError::NewNameEmpty => err!(FileNameEmpty),
            RenameFileError::CannotRenameRoot => err!(NoRootOps("rename".to_string())),
            RenameFileError::NewNameContainsSlash => {
                err!(FileNameHasSlash(new_name.to_string()))
            }
            RenameFileError::FileNameNotAvailable => {
                err!(FileNameNotAvailable(new_name.to_string()))
            }
            RenameFileError::FileDoesNotExist => err_unexpected!("FileDoesNotExist!"),
        },
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })
}
