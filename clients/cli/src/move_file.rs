use crate::utils::{exit_success, get_config};
use crate::{err_extra, err_unexpected, exitlb};
use lockbook_core::{get_file_by_path, Error as CoreError, GetFileByPathError, MoveFileError};

pub fn move_file(path1: &str, path2: &str) {
    match get_file_by_path(&get_config(), path1) {
        Ok(file_metadata) => match get_file_by_path(&get_config(), path2) {
            Ok(target_file_metadata) => {
                match lockbook_core::move_file(
                    &get_config(),
                    file_metadata.id,
                    target_file_metadata.id,
                ) {
                    Ok(_) => exit_success(""),
                    Err(move_file_error) => match move_file_error {
                        CoreError::UiError(MoveFileError::NoAccount) => exitlb!(NoAccount),
                        CoreError::UiError(MoveFileError::CannotMoveRoot) => {
                            exitlb!(NoRootOps("move".to_string()))
                        }
                        CoreError::UiError(MoveFileError::FileDoesNotExist) => {
                            exitlb!(FileNotFound(path1.to_string()))
                        }
                        CoreError::UiError(MoveFileError::TargetParentDoesNotExist) => {
                            exitlb!(FileNotFound(path2.to_string()))
                        }
                        CoreError::UiError(MoveFileError::FolderMovedIntoItself) => {
                            exitlb!(CannotMoveFolderIntoItself)
                        }
                        CoreError::UiError(MoveFileError::TargetParentHasChildNamedThat) => {
                            exitlb!(FileNameNotAvailable(target_file_metadata.name))
                        }
                        CoreError::UiError(MoveFileError::DocumentTreatedAsFolder) => err_extra!(
                            DocTreatedAsFolder(path2.to_string()),
                            "{} cannot be moved to {}",
                            file_metadata.name,
                            target_file_metadata.name
                        ).exit(),
                        CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
                    },
                }
            }
            Err(get_file_error) => match get_file_error {
                CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                    err_unexpected!("No file at {}", path2).exit()
                }
                CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
            },
        },
        Err(get_file_error) => match get_file_error {
            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                exitlb!(FileNotFound(path1.to_string()))
            }
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
        },
    }
}
