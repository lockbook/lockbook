use lockbook_core::model::file_metadata::FileType::Folder;
use lockbook_core::{create_file_at_path, CreateFileAtPathError, Error as CoreError};
use std::fs;
use std::fs::File;
use std::path::Path;
use uuid::Uuid;

use crate::utils::{
    edit_file_with_editor, exit_success, get_account_or_exit, get_config, save_temp_file_contents,
    set_up_auto_save, stop_auto_save,
};
use crate::{err_unexpected, exitlb};

pub fn new(file_name: &str) {
    get_account_or_exit();

    let file_metadata = match create_file_at_path(&get_config(), &file_name) {
        Ok(file_metadata) => file_metadata,
        Err(err) => match err {
            CoreError::UiError(CreateFileAtPathError::FileAlreadyExists) => {
                exitlb!(FileAlreadyExists(file_name.to_string()))
            }
            CoreError::UiError(CreateFileAtPathError::NoAccount) => exitlb!(NoAccount),
            CoreError::UiError(CreateFileAtPathError::NoRoot) => exitlb!(NoRoot),
            CoreError::UiError(CreateFileAtPathError::PathContainsEmptyFile) => {
                exitlb!(PathContainsEmptyFile(file_name.to_string()))
            }
            CoreError::UiError(CreateFileAtPathError::PathDoesntStartWithRoot) => {
                exitlb!(PathNoRoot(file_name.to_string()))
            }
            CoreError::UiError(CreateFileAtPathError::DocumentTreatedAsFolder) => {
                exitlb!(DocTreatedAsFolder(file_name.to_string()))
            }
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
        },
    };

    let directory_location = format!("/tmp/{}", Uuid::new_v4().to_string());
    fs::create_dir(&directory_location).unwrap_or_else(|err| {
        err_unexpected!("couldn't open temporary file for writing: {:#?}", err).exit()
    });
    let file_location = format!("{}/{}", directory_location, file_metadata.name);
    let temp_file_path = Path::new(file_location.as_str());
    match File::create(&temp_file_path) {
        Ok(_) => {}
        Err(err) => err_unexpected!("couldn't open temporary file for writing: {:#?}", err).exit(),
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
