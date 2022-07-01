use std::fs;
use std::io::Write;

use lockbook_core::Error as LbError;
use lockbook_core::GetFileByPathError;
use lockbook_core::ReadDocumentError;
use lockbook_core::{Core, Error, GetFileByIdError};

use crate::error::CliError;
use crate::utils::{
    edit_file_with_editor, get_directory_location, save_temp_file_contents, set_up_auto_save,
    stop_auto_save,
};
use crate::Uuid;

pub fn edit(core: &Core, lb_path: Option<String>, id: Option<Uuid>) -> Result<(), CliError> {
    core.get_account()?;

    let file_metadata = match (lb_path, id) {
        (Some(path), None) => core.get_by_path(&path).map_err(|err| match err {
            LbError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                CliError::file_not_found(&path)
            }
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        }),
        (None, Some(id)) => core.get_file_by_id(id).map_err(|err| match err {
            Error::UiError(GetFileByIdError::NoFileWithThatId) => {
                CliError::unexpected(format!("No file with id {}", id))
            }
            Error::Unexpected(msg) => CliError::unexpected(msg),
        }),
        (Some(_), Some(_)) => {
            Err(CliError::input(format!("Provided both a path and an ID, only one is needed!")))
        }
        (None, None) => Err(CliError::input(format!("Either a path or an input is required!"))),
    }?;

    let file_content = core
        .read_document(file_metadata.id)
        .map_err(|err| match err {
            LbError::UiError(ReadDocumentError::TreatedFolderAsDocument) => {
                CliError::dir_treated_as_doc(&file_metadata)
            }
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
