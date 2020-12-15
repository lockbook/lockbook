use lockbook_core::model::file_metadata::FileType::Folder;
use lockbook_core::{create_file_at_path, CreateFileAtPathError, Error as CoreError};
use std::fs;
use std::fs::File;
use std::path::Path;
use uuid::Uuid;

use crate::edit::save_file_to_core;
use crate::utils::{edit_file_with_editor, exit_with, exit_with_no_account, get_account_or_exit, get_config, set_up_auto_save, stop_auto_save};
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

    set_up_auto_save(file_metadata.clone(), file_location.clone());

    if file_metadata.file_type == Folder {
        exit_with("Folder created.", SUCCESS);
    }

    let watcher = set_up_auto_save(file_metadata.clone(), file_location.clone());

    let edit_was_successful = edit_file_with_editor(&file_location);

    stop_auto_save(watcher, file_location.clone());

    if edit_was_successful {
        save_file_to_core(file_metadata, &file_location, temp_file_path, false)
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
    }

    fs::remove_file(&temp_file_path)
        .unwrap_or_else(|_| panic!("Failed to delete temporary file: {}", &file_location));
}
