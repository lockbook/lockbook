use crate::error::CliResult;
use crate::utils::{account, config};
use crate::{err, err_unexpected};
use lockbook_core::model::errors::GetFileByPathError;
use lockbook_core::model::errors::RenameFileError;
use lockbook_core::{get_file_by_path, rename_file, Error as CoreError};

pub fn rename(path: &str, new_name: &str) -> CliResult<()> {
    account()?;

    let file_metadata = get_file_by_path(&config()?, path).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(path.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    rename_file(&config()?, file_metadata.id, new_name).map_err(|err| match err {
        CoreError::UiError(err) => match err {
            RenameFileError::NewNameEmpty => err!(FileNameEmpty),
            RenameFileError::CannotRenameRoot => err!(NoRootOps("rename")),
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
