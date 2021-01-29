use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_core::{
    get_file_by_path, read_document, Error as CoreError, GetFileByPathError, ReadDocumentError,
};

use crate::exitlb;
use crate::utils::{
    edit_file_with_editor, get_account_or_exit, get_config, save_temp_file_contents,
    set_up_auto_save, stop_auto_save,
};

pub fn edit(file_name: &str) {
    get_account_or_exit();

    let file_metadata = match get_file_by_path(&get_config(), file_name) {
        Ok(file_metadata) => file_metadata,
        Err(err) => match err {
            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                exitlb!(FileNotFound, "No file found with the path {}", file_name)
            }
            CoreError::Unexpected(msg) => exitlb!(Unexpected, "{}", msg),
        },
    };

    let file_content = match read_document(&get_config(), file_metadata.id) {
        Ok(content) => content,
        Err(err) => match err {
            CoreError::UiError(ReadDocumentError::TreatedFolderAsDocument) => {
                exitlb!(DocTreatedAsFolder(file_name.to_string()))
            }
            CoreError::UiError(ReadDocumentError::NoAccount)
            | CoreError::UiError(ReadDocumentError::FileDoesNotExist)
            | CoreError::Unexpected(_) => exitlb!(
                Unexpected,
                "Unexpected error while reading encrypted doc: {:#?}",
                err
            ),
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
    let mut file_handle = match File::create(&temp_file_path) {
        Ok(handle) => handle,
        Err(err) => exitlb!(
            Unexpected,
            "Could not open temporary file for writing. OS: {:#?}",
            err
        ),
    };

    file_handle.write_all(&file_content).unwrap_or_else(|_| {
        exitlb!(
            OsCouldNotWriteFile,
            "Failed to write decrypted contents to temporary file, check {}",
            file_location
        )
    });

    file_handle.sync_all().unwrap_or_else(|_| {
        exitlb!(
            OsCouldNotWriteFile,
            "Failed to write decrypted contents to temporary file, check {}",
            file_location
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
        exitlb!(
            OsCouldNotDeleteFile,
            "Failed to delete temporary file: {}",
            file_location
        )
    });
}
