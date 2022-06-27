use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_core::GetFileByPathError;
use lockbook_core::MoveFileError;

use crate::error::CliError;

pub fn move_file(core: &Core, path1: &str, path2: &str) -> Result<(), CliError> {
    core.get_account()?;

    let file_metadata = core.get_by_path(path1).map_err(|err| match err {
        LbError::UiError(GetFileByPathError::NoFileAtThatPath) => CliError::file_not_found(path1),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    let target_file_metadata = core.get_by_path(path2).map_err(|err| match err {
        LbError::UiError(GetFileByPathError::NoFileAtThatPath) => CliError::file_not_found(path2),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    core.move_file(file_metadata.id, target_file_metadata.id)
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                MoveFileError::CannotMoveRoot => CliError::no_root_ops("move"),
                MoveFileError::FileDoesNotExist => CliError::file_not_found(path1),
                MoveFileError::TargetParentDoesNotExist => CliError::file_not_found(path2),
                MoveFileError::FolderMovedIntoItself => CliError::moving_folder_into_itself(),
                MoveFileError::TargetParentHasChildNamedThat => CliError::file_name_taken(""), //todo
                MoveFileError::DocumentTreatedAsFolder => CliError::doc_treated_as_dir(path2)
                    .with_extra(format!(
                        "{} cannot be moved to {}",
                        file_metadata.decrypted_name, target_file_metadata.decrypted_name
                    )),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })
}
