use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_core::{
    get_file_by_path, read_document, Error as CoreError, GetFileByPathError, ReadDocumentError,
};

use crate::error::CliResult;
use crate::utils::{
    edit_file_with_editor, get_account_or_exit, get_config, save_temp_file_contents,
    set_up_auto_save, stop_auto_save,
};
use crate::{err, err_unexpected};

pub fn edit(file_name: &str) -> CliResult<()> {
    get_account_or_exit();

    let file_metadata = get_file_by_path(&get_config(), file_name).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(file_name.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let file_content = read_document(&get_config(), file_metadata.id).map_err(|err| match err {
        CoreError::UiError(ReadDocumentError::TreatedFolderAsDocument) => {
            err!(DocTreatedAsFolder(file_name.to_string()))
        }
        CoreError::UiError(ReadDocumentError::NoAccount)
        | CoreError::UiError(ReadDocumentError::FileDoesNotExist)
        | CoreError::Unexpected(_) => err_unexpected!("reading encrypted doc: {:#?}", err),
    })?;

    let directory_location = format!("/tmp/{}", Uuid::new_v4().to_string());
    fs::create_dir(&directory_location)
        .map_err(|err| err_unexpected!("couldn't open temporary file for writing: {:#?}", err))?;

    let file_location = format!("{}/{}", directory_location, file_metadata.name);
    let temp_file_path = Path::new(file_location.as_str());
    let mut file_handle = File::create(&temp_file_path)
        .map_err(|err| err_unexpected!("couldn't open temporary file for writing: {:#?}", err))?;

    file_handle
        .write_all(&file_content)
        .map_err(|err| err!(OsCouldNotWriteFile(file_location.clone(), err)))?;

    file_handle
        .sync_all()
        .map_err(|err| err!(OsCouldNotWriteFile(file_location.clone(), err)))?;

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

    fs::remove_file(&temp_file_path).map_err(|err| err!(OsCouldNotDeleteFile(file_location, err)))
}
