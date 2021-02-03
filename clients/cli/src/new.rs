use lockbook_core::model::file_metadata::FileType::Folder;
use lockbook_core::{create_file_at_path, CreateFileAtPathError, Error as CoreError};
use std::fs;
use std::fs::File;
use std::path::Path;
use uuid::Uuid;

use crate::error::CliResult;
use crate::utils::{
    edit_file_with_editor, exit_success, get_account_or_exit, get_config, save_temp_file_contents,
    set_up_auto_save, stop_auto_save,
};
use crate::{err, err_unexpected};

pub fn new(file_name: &str) -> CliResult {
    get_account_or_exit();
    let cfg = get_config();

    let file_metadata = create_file_at_path(&cfg, &file_name).map_err(|err| match err {
        CoreError::UiError(err) => match err {
            CreateFileAtPathError::FileAlreadyExists => {
                err!(FileAlreadyExists(file_name.to_string()))
            }
            CreateFileAtPathError::NoAccount => err!(NoAccount),
            CreateFileAtPathError::NoRoot => err!(NoRoot),
            CreateFileAtPathError::PathContainsEmptyFile => {
                err!(PathContainsEmptyFile(file_name.to_string()))
            }
            CreateFileAtPathError::PathDoesntStartWithRoot => {
                err!(PathNoRoot(file_name.to_string()))
            }
            CreateFileAtPathError::DocumentTreatedAsFolder => {
                err!(DocTreatedAsFolder(file_name.to_string()))
            }
        },
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let directory_location = format!("/tmp/{}", Uuid::new_v4().to_string());
    fs::create_dir(&directory_location)
        .map_err(|err| err_unexpected!("couldn't open temporary file for writing: {:#?}", err))?;

    let file_location = format!("{}/{}", directory_location, file_metadata.name);
    let temp_file_path = Path::new(file_location.as_str());
    let _ = File::create(&temp_file_path)
        .map_err(|err| err_unexpected!("couldn't open temporary file for writing: {:#?}", err))?;

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
        .map_err(|err| err_unexpected!("deleting temporary file '{}': {}", &file_location, err))
}
