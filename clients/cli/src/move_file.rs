use crate::exitlb;
use crate::utils::{exit_success, exit_with_no_account, get_config};
use lockbook_core::{
    get_file_by_path, Error as CoreError, Error, GetFileByPathError, MoveFileError,
};

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
                        CoreError::UiError(MoveFileError::NoAccount) => exit_with_no_account(),
                        CoreError::UiError(MoveFileError::CannotMoveRoot) => {
                            exitlb!(NoRootOps, "Cannot move root directory!")
                        }
                        CoreError::UiError(MoveFileError::FileDoesNotExist) => {
                            exitlb!(FileNotFound, "No file found at {}", path1)
                        }
                        CoreError::UiError(MoveFileError::TargetParentDoesNotExist) => {
                            exitlb!(FileNotFound, "No file found at {}", path2)
                        }
                        Error::UiError(MoveFileError::FolderMovedIntoItself) => {
                            exitlb!(
                                CannotMoveFolderIntoItself,
                                "Cannot move file into its self or children."
                            )
                        }
                        CoreError::UiError(MoveFileError::TargetParentHasChildNamedThat) => {
                            exitlb!(
                                FileNameNotAvailable,
                                "{}/ has a file named {}",
                                file_metadata.name,
                                target_file_metadata.name
                            )
                        }
                        CoreError::UiError(MoveFileError::DocumentTreatedAsFolder) => exitlb!(
                            DocTreatedAsFolder(path2.to_string()),
                            "{} cannot be moved to {}",
                            file_metadata.name,
                            target_file_metadata.name
                        ),
                        CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
                    },
                }
            }
            Err(get_file_error) => match get_file_error {
                CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                    exitlb!(Unexpected, "No file at {}", path2)
                }
                CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
            },
        },
        Err(get_file_error) => match get_file_error {
            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                exitlb!(FileNotFound, "No file at {}", path1)
            }
            CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
        },
    }
}
