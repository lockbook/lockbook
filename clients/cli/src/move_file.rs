use crate::utils::{exit_with, exit_with_no_account, get_config};
use crate::{
    DOCUMENT_TREATED_AS_FOLDER, FILE_NAME_NOT_AVAILABLE, FILE_NOT_FOUND, NO_ROOT_OPS,
    UNEXPECTED_ERROR,
};
use lockbook_core::{get_file_by_path, GetFileByPathError, MoveFileError};
use std::process::exit;

pub fn move_file(path1: &str, path2: &str) {
    match get_file_by_path(&get_config(), path1) {
        Ok(file_metadata) => match get_file_by_path(&get_config(), path2) {
            Ok(target_file_metadata) => {
                match lockbook_core::move_file(
                    &get_config(),
                    file_metadata.id,
                    target_file_metadata.id,
                ) {
                    Ok(_) => exit(0),
                    Err(move_file_error) => match move_file_error {
                        MoveFileError::NoAccount => exit_with_no_account(),
                        MoveFileError::CannotMoveRoot => {
                            exit_with("Cannot move root directory!", NO_ROOT_OPS)
                        }
                        MoveFileError::FileDoesNotExist => {
                            exit_with(&format!("No file found at {}", path1), FILE_NOT_FOUND)
                        }
                        MoveFileError::TargetParentDoesNotExist => {
                            exit_with(&format!("No file found at {}", path2), FILE_NOT_FOUND)
                        }
                        MoveFileError::TargetParentHasChildNamedThat => exit_with(
                            &format!(
                                "{}/ has a file named {}",
                                file_metadata.name, target_file_metadata.name
                            ),
                            FILE_NAME_NOT_AVAILABLE,
                        ),
                        MoveFileError::DocumentTreatedAsFolder => exit_with(
                            &format!(
                                "{} is a document, {} cannot be moved there",
                                target_file_metadata.name, file_metadata.name
                            ),
                            DOCUMENT_TREATED_AS_FOLDER,
                        ),
                        MoveFileError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
                    },
                }
            }
            Err(get_file_error) => match get_file_error {
                GetFileByPathError::NoFileAtThatPath => {
                    exit_with(&format!("No file at {}", path2), FILE_NOT_FOUND)
                }
                GetFileByPathError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            },
        },
        Err(get_file_error) => match get_file_error {
            GetFileByPathError::NoFileAtThatPath => {
                exit_with(&format!("No file at {}", path1), FILE_NOT_FOUND)
            }
            GetFileByPathError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }
}
