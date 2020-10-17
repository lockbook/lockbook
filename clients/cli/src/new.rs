use std::fs;
use std::fs::File;
use std::path::Path;

use lockbook_core::model::crypto::DecryptedValue;
use lockbook_core::model::file_metadata::FileType::Folder;
use lockbook_core::{
    create_file_at_path, write_document, CreateFileAtPathError, Error as CoreError,
    WriteToDocumentError,
};
use uuid::Uuid;

use crate::utils::{
    edit_file_with_editor, exit_with, exit_with_no_account, get_account_or_exit, get_config,
};
use crate::{
    DOCUMENT_TREATED_AS_FOLDER, FILE_ALREADY_EXISTS, NO_ROOT, PATH_CONTAINS_EMPTY_FILE,
    PATH_NO_ROOT, SUCCESS, UNEXPECTED_ERROR,
};

pub fn new(file_name: &str) {
    get_account_or_exit();

    let file_metadata = match create_file_at_path(&get_config(), &file_name) {
        Ok(file_metadata) => file_metadata,
        Err(err) => match err {
            CoreError::UiError(CreateFileAtPathError::FileAlreadyExists) => {
                exit_with("File already exists!", FILE_ALREADY_EXISTS)
            }
            CoreError::UiError(CreateFileAtPathError::NoAccount) => exit_with_no_account(),
            CoreError::UiError(CreateFileAtPathError::NoRoot) => {
                exit_with("No root folder, have you synced yet?", NO_ROOT)
            }
            CoreError::UiError(CreateFileAtPathError::PathContainsEmptyFile) => {
                exit_with("Path contains an empty file.", PATH_CONTAINS_EMPTY_FILE)
            }
            CoreError::UiError(CreateFileAtPathError::PathDoesntStartWithRoot) => {
                exit_with("Path doesn't start with your root folder.", PATH_NO_ROOT)
            }
            CoreError::UiError(CreateFileAtPathError::DocumentTreatedAsFolder) => exit_with(
                "A file within your path is a document that was treated as a folder",
                DOCUMENT_TREATED_AS_FOLDER,
            ),
            CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
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
    match File::create(&temp_file_path) {
        Ok(_) => {}
        Err(err) => exit_with(
            &format!("Could not open temporary file for writing. OS: {:#?}", err),
            UNEXPECTED_ERROR,
        ),
    }

    if file_metadata.file_type == Folder {
        exit_with("Folder created.", SUCCESS);
    }

    let edit_was_successful = edit_file_with_editor(&file_location);

    if edit_was_successful {
        let secret = match fs::read_to_string(temp_file_path) {
            Ok(content) => DecryptedValue::from(content),
            Err(err) => exit_with(
                &format!(
                    "Could not read from temporary file, not deleting {}, err: {:#?}",
                    file_location, err
                ),
                UNEXPECTED_ERROR,
            ),
        };

        match write_document(&get_config(), file_metadata.id, &secret) {
            Ok(_) => exit_with(
                "Document encrypted and saved. Cleaning up temporary file.",
                SUCCESS,
            ),
            Err(err) => match err {
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
            },
        }
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
    }

    fs::remove_file(&temp_file_path)
        .unwrap_or_else(|_| panic!("Failed to delete temporary file: {}", &file_location));
}
