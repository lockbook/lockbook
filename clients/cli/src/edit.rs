use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::fs;

use uuid::Uuid;

use lockbook_core::{
    get_file_by_path, read_document, write_document, Error as CoreError, GetFileByPathError,
    ReadDocumentError, WriteToDocumentError,
};

use crate::utils::{
    edit_file_with_editor, exit_with, get_account_or_exit, get_config, set_up_auto_save,
};
use crate::{
    COULD_NOT_DELETE_OS_FILE, COULD_NOT_READ_OS_FILE, COULD_NOT_WRITE_TO_OS_FILE,
    DOCUMENT_TREATED_AS_FOLDER, FILE_NOT_FOUND, SUCCESS, UNEXPECTED_ERROR,
};
use lockbook_core::model::file_metadata::FileMetadata;

pub fn edit(file_name: &str) {
    get_account_or_exit();

    let file_metadata = match get_file_by_path(&get_config(), file_name) {
        Ok(file_metadata) => file_metadata,
        Err(err) => match err {
            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => exit_with(
                &format!("No file found with the path {}", file_name),
                FILE_NOT_FOUND,
            ),
            CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    };

    let file_content = match read_document(&get_config(), file_metadata.id) {
        Ok(content) => content,
        Err(error) => match error {
            CoreError::UiError(ReadDocumentError::TreatedFolderAsDocument) => {
                exit_with("Specified file is a folder!", DOCUMENT_TREATED_AS_FOLDER)
            }
            CoreError::UiError(ReadDocumentError::NoAccount)
            | CoreError::UiError(ReadDocumentError::FileDoesNotExist)
            | CoreError::Unexpected(_) => exit_with(
                &format!("Unexpected error while reading encrypted doc: {:#?}", error),
                UNEXPECTED_ERROR,
            ),
        },
    };

    let directory_location = format!("/tmp/{}", Uuid::new_v4().to_string());
    fs::create_dir(&directory_location).unwrap_or_else(|err| {
        exit_with(
            &format!("Could not open temporary file for writing. OS: {:#?}", err),
            UNEXPECTED_ERROR,
        )
    });
    let file_location = format!("{}/{}", directory_location, file_metadata.name);
    let temp_file_path = Path::new(file_location.as_str());
    let mut file_handle = match File::create(&temp_file_path) {
        Ok(handle) => handle,
        Err(err) => exit_with(
            &format!("Could not open temporary file for writing. OS: {:#?}", err),
            UNEXPECTED_ERROR,
        ),
    };

    file_handle.write_all(&file_content).unwrap_or_else(|_| {
        exit_with(
            &format!(
                "Failed to write decrypted contents to temporary file, check {}",
                file_location
            ),
            COULD_NOT_WRITE_TO_OS_FILE,
        )
    });

    file_handle.sync_all().unwrap_or_else(|_| {
        exit_with(
            &format!(
                "Failed to write decrypted contents to temporary file, check {}",
                file_location
            ),
            COULD_NOT_WRITE_TO_OS_FILE,
        )
    });

    set_up_auto_save(file_metadata.clone(), file_location.clone());

    let edit_was_successful = edit_file_with_editor(&file_location);

    if edit_was_successful {
        let secret = fs::read_to_string(temp_file_path).unwrap_or_else(|_| {
            exit_with(
                &format!(
                    "Failed to read from temporary file, check {}",
                    file_location
                ),
                COULD_NOT_READ_OS_FILE,
            )
        });

        match write_document(&get_config(), file_metadata.id, secret.as_bytes()) {
            Ok(_) => exit_with(
                "Document encrypted and saved. Cleaning up temporary file.",
                SUCCESS,
            ),
            Err(err) => match err {
                CoreError::UiError(WriteToDocumentError::NoAccount)
                | CoreError::UiError(WriteToDocumentError::FileDoesNotExist)
                | CoreError::UiError(WriteToDocumentError::FolderTreatedAsDocument)
                | CoreError::Unexpected(_) => exit_with(
                    &format!("Unexpected error saving file: {:#?}", err),
                    UNEXPECTED_ERROR,
                ),
            },
        }
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
    }

    fs::remove_file(&temp_file_path).unwrap_or_else(|_| {
        exit_with(
            &format!("Failed to delete temporary file: {}", &file_location).as_str(),
            COULD_NOT_DELETE_OS_FILE,
        )
    });
}

pub fn save_file_to_core(
    file_metadata: FileMetadata,
    file_location: &String,
    temp_file_path: &Path,
    silent: bool,
) {
    let secret = match fs::read_to_string(temp_file_path) {
        Ok(content) => content.into_bytes(),
        Err(err) => {
            if !silent {
                exit_with(
                    &format!(
                        "Could not read from temporary file, not deleting {}, err: {:#?}",
                        file_location, err
                    ),
                    UNEXPECTED_ERROR,
                )
            } else {
                return;
            }
        }
    };

    match write_document(&get_config(), file_metadata.id, &secret) {
        Ok(_) => {
            if !silent {
                exit_with(
                    "Document encrypted and saved. Cleaning up temporary file.",
                    SUCCESS,
                )
            } else {
                return;
            }
        }
        Err(err) => {
            if !silent {
                match err {
                    CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
                    CoreError::UiError(WriteToDocumentError::NoAccount) => exit_with(
                        "Unexpected: No account! Run init or import to get started!",
                        UNEXPECTED_ERROR,
                    ),
                    CoreError::UiError(WriteToDocumentError::FileDoesNotExist) => {
                        exit_with("Unexpected: FileDoesNotExist", UNEXPECTED_ERROR)
                    }
                    CoreError::UiError(WriteToDocumentError::FolderTreatedAsDocument) => {
                        exit_with("Unexpected: CannotWriteToFolder", UNEXPECTED_ERROR)
                    }
                }
            } else {
                return;
            }
        }
    }
}