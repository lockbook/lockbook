use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_core::{
    get_file_by_path, read_document, Error as CoreError, GetFileByPathError, ReadDocumentError,
};

use crate::utils::{
    edit_file_with_editor, exit_with, get_account_or_exit, get_config, save_temp_file_contents,
    set_up_auto_save, stop_auto_save,
};
use crate::{
    COULD_NOT_DELETE_OS_FILE, COULD_NOT_WRITE_TO_OS_FILE, DOCUMENT_TREATED_AS_FOLDER,
    FILE_NOT_FOUND, UNEXPECTED_ERROR,
};

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

    let watcher = set_up_auto_save(file_metadata.clone(), file_location.clone());

    let edit_was_successful = edit_file_with_editor(&file_location);

    if let Some(ok) = watcher {
        stop_auto_save(ok, file_location.clone());
    }

    if edit_was_successful {
        save_temp_file_contents(file_metadata, &file_location, temp_file_path, false)
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
