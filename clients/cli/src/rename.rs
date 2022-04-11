use lockbook_core::model::errors::GetFileByPathError;
use lockbook_core::model::errors::RenameFileError;
use lockbook_core::Error as LbError;
use lockbook_core::LbCore;

use crate::error::CliError;

pub fn rename(core: &LbCore, path: &str, new_name: &str) -> Result<(), CliError> {
    core.get_account()?;

    let target_id = core
        .get_by_path(path)
        .map(|meta| meta.id)
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                GetFileByPathError::NoFileAtThatPath => CliError::file_not_found(path),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })?;

    core.rename_file(target_id, new_name)
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                RenameFileError::NewNameEmpty => CliError::file_name_empty(),
                RenameFileError::CannotRenameRoot => CliError::no_root_ops("rename"),
                RenameFileError::NewNameContainsSlash => CliError::file_name_has_slash(new_name),
                RenameFileError::FileNameNotAvailable => CliError::file_name_taken(new_name),
                RenameFileError::FileDoesNotExist => CliError::file_not_found(path),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })
}
