use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_core::model::crypto::DecryptedValue;
use lockbook_core::{
    get_file_by_path, read_document, write_document, GetFileByPathError, ReadDocumentError,
    WriteToDocumentError,
};

use crate::utils::{edit_file_with_editor, exit_with, get_account_or_exit, get_config};
use crate::{
    COULD_NOT_DELETE_OS_FILE, COULD_NOT_READ_OS_FILE, COULD_NOT_WRITE_TO_OS_FILE,
    DOCUMENT_TREATED_AS_FOLDER, FILE_NOT_FOUND, SUCCESS, UNEXPECTED_ERROR,
};

pub fn edit(file_name: &str) {
    get_account_or_exit();

    let file_metadata = match get_file_by_path(&get_config(), file_name) {
        Ok(file_metadata) => file_metadata,
        Err(err) => match err {
            GetFileByPathError::NoFileAtThatPath => exit_with(
                &format!("No file found with the path {}", file_name),
                FILE_NOT_FOUND,
            ),
            GetFileByPathError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    };

    let file_content = match read_document(&get_config(), file_metadata.id) {
        Ok(content) => content,
        Err(error) => match error {
            ReadDocumentError::TreatedFolderAsDocument => {
                exit_with("Specified file is a folder!", DOCUMENT_TREATED_AS_FOLDER)
            }
            ReadDocumentError::NoAccount
            | ReadDocumentError::FileDoesNotExist
            | ReadDocumentError::UnexpectedError(_) => exit_with(
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

    file_handle
        .write_all(&file_content.secret.into_bytes())
        .unwrap_or_else(|_| {
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

        match write_document(
            &get_config(),
            file_metadata.id,
            &DecryptedValue::from(secret),
        ) {
            Ok(_) => exit_with(
                "Document encrypted and saved. Cleaning up temporary file.",
                SUCCESS,
            ),
            Err(err) => match err {
                WriteToDocumentError::NoAccount
                | WriteToDocumentError::FileDoesNotExist
                | WriteToDocumentError::FolderTreatedAsDocument
                | WriteToDocumentError::UnexpectedError(_) => exit_with(
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
