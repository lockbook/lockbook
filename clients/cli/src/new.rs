use lockbook_core::{create_file_at_path, CreateFileAtPathError, Error as CoreError};
use lockbook_models::file_metadata::FileType::Folder;
use std::fs;
use std::fs::File;
use std::path::Path;

use crate::error::CliResult;
use crate::utils::{
    account, config, edit_file_with_editor, get_directory_location, save_temp_file_contents,
    set_up_auto_save, stop_auto_save,
};
use crate::{err, err_unexpected};

pub fn new(file_name: &str) -> CliResult<()> {
    account()?;
    let cfg = config()?;

    let file_metadata = create_file_at_path(&cfg, file_name).map_err(|err| match err {
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

    let file_location = format!(
        "{}/{}",
        get_directory_location()?,
        file_metadata.decrypted_name
    );
    let temp_file_path = Path::new(&file_location);
    let _ = File::create(&temp_file_path)
        .map_err(|err| err_unexpected!("couldn't open temporary file for writing: {:#?}", err))?;

    if file_metadata.file_type == Folder {
        println!("Folder created.");
        return Ok(());
    }

    let watcher = set_up_auto_save(file_metadata.id, file_location.clone());

    let edit_was_successful = edit_file_with_editor(&file_location);

    if let Some(ok) = watcher {
        stop_auto_save(ok, file_location.clone());
    }

    if edit_was_successful {
        match save_temp_file_contents(file_metadata.id, &file_location) {
            Ok(_) => println!("Document encrypted and saved. Cleaning up temporary file."),
            Err(err) => err.print(),
        }
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
    }

    fs::remove_file(&temp_file_path)
        .map_err(|err| err_unexpected!("deleting temporary file '{}': {}", &file_location, err))
}
