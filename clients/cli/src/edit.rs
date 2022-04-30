use std::fs;
use std::io::Write;

use lockbook_core::model::errors::GetFileByPathError;
use lockbook_core::model::errors::ReadDocumentError;
use lockbook_core::Core;
use lockbook_core::Error as LbError;

use crate::error::CliError;
use crate::utils::{
    edit_file_with_editor, get_directory_location, save_temp_file_contents, set_up_auto_save,
    stop_auto_save,
};

pub fn edit(core: &Core, lb_path: &str) -> Result<(), CliError> {
    core.get_account()?;

    let file_metadata = core.get_by_path(lb_path).map_err(|err| match err {
        LbError::UiError(GetFileByPathError::NoFileAtThatPath) => CliError::file_not_found(lb_path),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    let file_content = core
        .read_document(file_metadata.id)
        .map_err(|err| match err {
            LbError::UiError(ReadDocumentError::TreatedFolderAsDocument) => {
                CliError::dir_treated_as_doc(lb_path)
            }
            LbError::UiError(ReadDocumentError::NoAccount) => CliError::no_account(),
            LbError::UiError(ReadDocumentError::FileDoesNotExist) | LbError::Unexpected(_) => {
                CliError::unexpected(format!("reading encrypted doc: {:#?}", err))
            }
        })?;

    let mut temp_file_path = get_directory_location()?;
    temp_file_path.push(file_metadata.decrypted_name);

    let mut file_handle = fs::File::create(&temp_file_path).map_err(|err| {
        CliError::unexpected(format!("couldn't open temporary file for writing: {:#?}", err))
    })?;

    file_handle
        .write_all(&file_content)
        .map_err(|err| CliError::os_write_file(&temp_file_path, err))?;

    file_handle
        .sync_all()
        .map_err(|err| CliError::os_write_file(&temp_file_path, err))?;

    let watcher = set_up_auto_save(core, file_metadata.id, &temp_file_path);

    let edit_was_successful = edit_file_with_editor(&temp_file_path);

    if let Some(ok) = watcher {
        stop_auto_save(ok, &temp_file_path);
    }

    if edit_was_successful {
        match save_temp_file_contents(core, file_metadata.id, &temp_file_path) {
            Ok(_) => println!("Document encrypted and saved. Cleaning up temporary file."),
            Err(err) => err.print(),
        }
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
    }

    fs::remove_file(&temp_file_path).map_err(|err| CliError::os_delete_file(&temp_file_path, err))
}
