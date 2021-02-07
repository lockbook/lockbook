use crate::error::CliResult;
use crate::utils::get_config;
use crate::{err, err_extra, err_unexpected};
use lockbook_core::{get_file_by_path, Error as CoreError, GetFileByPathError, MoveFileError};

pub fn move_file(path1: &str, path2: &str) -> CliResult<()> {
    let cfg = get_config();

    let file_metadata = get_file_by_path(&cfg, path1).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(path1.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let target_file_metadata = get_file_by_path(&cfg, path2).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err_unexpected!("No file at {}", path2)
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    lockbook_core::move_file(&cfg, file_metadata.id, target_file_metadata.id).map_err(|err| {
        match err {
            CoreError::UiError(MoveFileError::NoAccount) => err!(NoAccount),
            CoreError::UiError(MoveFileError::CannotMoveRoot) => err!(NoRootOps("move")),
            CoreError::UiError(MoveFileError::FileDoesNotExist) => {
                err!(FileNotFound(path1.to_string()))
            }
            CoreError::UiError(MoveFileError::TargetParentDoesNotExist) => {
                err!(FileNotFound(path2.to_string()))
            }
            CoreError::UiError(MoveFileError::FolderMovedIntoItself) => {
                err!(CannotMoveFolderIntoItself)
            }
            CoreError::UiError(MoveFileError::TargetParentHasChildNamedThat) => {
                err!(FileNameNotAvailable(target_file_metadata.name))
            }
            CoreError::UiError(MoveFileError::DocumentTreatedAsFolder) => err_extra!(
                DocTreatedAsFolder(path2.to_string()),
                "{} cannot be moved to {}",
                file_metadata.name,
                target_file_metadata.name
            ),
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
        }
    })
}
