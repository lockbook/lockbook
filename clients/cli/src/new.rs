use std::fs;

use lockbook_core::Error as LbError;
use lockbook_core::FileDeleteError;
use lockbook_core::{Core, Uuid};

use crate::error::CliError;
use crate::selector::create_meta;
use crate::utils::{
    edit_file_with_editor, get_directory_location, save_temp_file_contents, set_up_auto_save,
    stop_auto_save,
};

pub fn new(
    core: &Core, lb_path: Option<String>, parent: Option<Uuid>, name: Option<String>,
) -> Result<(), CliError> {
    core.get_account()?;

    let file = create_meta(core, lb_path, parent, name)?;

    let mut temp_file_path = get_directory_location()?;
    temp_file_path.push(&file.name);
    let _ = fs::File::create(&temp_file_path).map_err(|err| {
        CliError::unexpected(format!("couldn't open temporary file for writing: {:#?}", err))
    })?;

    if file.is_folder() {
        println!("Folder created.");
        return Ok(());
    }

    let watcher = set_up_auto_save(core, file.id, &temp_file_path);

    let edit_was_successful = edit_file_with_editor(&temp_file_path);

    if let Some(ok) = watcher {
        stop_auto_save(ok, &temp_file_path);
    }

    if edit_was_successful {
        match save_temp_file_contents(core, file.id, &temp_file_path) {
            Ok(_) => println!("Document encrypted and saved. Cleaning up temporary file."),
            Err(err) => err.print(),
        }
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
        let path = core.get_path_by_id(file.id)?;
        core.delete_file(file.id)
            .map_err(|err| match err {
                LbError::UiError(err) => match err {
                    FileDeleteError::FileDoesNotExist => CliError::file_not_found(path),
                    FileDeleteError::CannotDeleteRoot => CliError::no_root_ops("delete"),
                },
                LbError::Unexpected(msg) => CliError::unexpected(msg),
            })?;
    }

    fs::remove_file(&temp_file_path).map_err(|err| {
        CliError::unexpected(format!("deleting temporary file '{:?}': {}", &temp_file_path, err))
    })
}
