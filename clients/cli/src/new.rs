use lockbook_core::model::file_metadata::FileType::Folder;
use lockbook_core::{create_file_at_path, CreateFileAtPathError, Error as CoreError};
use std::fs;
use std::fs::File;
use std::path::Path;
use uuid::Uuid;

use crate::exitlb;
use crate::utils::{
    edit_file_with_editor, exit_success, exit_with_no_account, get_account_or_exit, get_config,
    save_temp_file_contents, set_up_auto_save, stop_auto_save,
};

pub fn new(file_name: &str) {
    get_account_or_exit();

    let file_metadata = match create_file_at_path(&get_config(), &file_name) {
        Ok(file_metadata) => file_metadata,
        Err(err) => match err {
            CoreError::UiError(CreateFileAtPathError::FileAlreadyExists) => {
                exitlb!(FileAlreadyExists(file_name.to_string()))
            }
            CoreError::UiError(CreateFileAtPathError::NoAccount) => exit_with_no_account(),
            CoreError::UiError(CreateFileAtPathError::NoRoot) => {
                exitlb!(NoRoot, "No root folder, have you synced yet?")
            }
            CoreError::UiError(CreateFileAtPathError::PathContainsEmptyFile) => {
                exitlb!(PathContainsEmptyFile, "Path contains an empty file.")
            }
            CoreError::UiError(CreateFileAtPathError::PathDoesntStartWithRoot) => {
                exitlb!(PathNoRoot, "Path doesn't start with your root folder.")
            }
            CoreError::UiError(CreateFileAtPathError::DocumentTreatedAsFolder) => exitlb!(
                DocTreatedAsFolder,
                "A file within your path is a document that was treated as a folder"
            ),
            CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
        },
    };

    let directory_location = format!("/tmp/{}", Uuid::new_v4().to_string());
    fs::create_dir(&directory_location).unwrap_or_else(|err| {
        exitlb!(
            Unexpected,
            "Could not open temporary file for writing. OS: {:#?}",
            err
        )
    });
    let file_location = format!("{}/{}", directory_location, file_metadata.name);
    let temp_file_path = Path::new(file_location.as_str());
    match File::create(&temp_file_path) {
        Ok(_) => {}
        Err(err) => exitlb!(
            Unexpected,
            "Could not open temporary file for writing. OS: {:#?}",
            err
        ),
    }

    if file_metadata.file_type == Folder {
        exit_success("Folder created.");
    }

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

    fs::remove_file(&temp_file_path)
        .unwrap_or_else(|_| panic!("Failed to delete temporary file: {}", &file_location));
}
