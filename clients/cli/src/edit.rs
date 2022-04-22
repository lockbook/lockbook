use std::fs;
use std::io::Write;

use lockbook_core::{
    get_file_by_path, read_document, Error as CoreError, GetFileByPathError, ReadDocumentError,
};

use crate::error::CliResult;
use crate::utils::{
    account, config, edit_file_with_editor, get_directory_location, save_temp_file_contents,
    set_up_auto_save, stop_auto_save,
};
use crate::{err, err_unexpected};

pub fn edit(file_name: &str) -> CliResult<()> {
    account()?;

    let file_metadata = get_file_by_path(&config()?, file_name).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(file_name.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let file_content = read_document(&config()?, file_metadata.id).map_err(|err| match err {
        CoreError::UiError(ReadDocumentError::TreatedFolderAsDocument) => {
            err!(FolderTreatedAsDoc(file_name.to_string()))
        }
        CoreError::UiError(ReadDocumentError::NoAccount)
        | CoreError::UiError(ReadDocumentError::FileDoesNotExist)
        | CoreError::Unexpected(_) => err_unexpected!("reading encrypted doc: {:#?}", err),
    })?;

    let mut file_buf = get_directory_location()?;
    file_buf.push(file_metadata.decrypted_name);

    let mut file_handle = fs::File::create(&file_buf)
        .map_err(|err| err_unexpected!("couldn't open temporary file for writing: {:#?}", err))?;

    file_handle
        .write_all(&file_content)
        .map_err(|err| err!(OsCouldNotWriteFile(file_buf.clone(), err)))?;

    file_handle
        .sync_all()
        .map_err(|err| err!(OsCouldNotWriteFile(file_buf.clone(), err)))?;

    let watcher = set_up_auto_save(file_metadata.id, file_buf.clone());

    let edit_was_successful = edit_file_with_editor(&file_buf);

    if let Some(ok) = watcher {
        stop_auto_save(ok, file_buf.clone());
    }

    if edit_was_successful {
        match save_temp_file_contents(file_metadata.id, &file_buf) {
            Ok(_) => println!("Document encrypted and saved. Cleaning up temporary file."),
            Err(err) => err.print(),
        }
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
    }

    fs::remove_file(&file_buf).map_err(|err| err!(OsCouldNotDeleteFile(file_buf, err)))
}
