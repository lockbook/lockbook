use std::fs;
use std::fs::File;
use std::path::Path;

use lockbook_core::model::errors::CreateFileAtPathError;
use lockbook_core::model::errors::FileDeleteError;
use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_models::file_metadata::FileType::Folder;

use crate::error::CliError;
use crate::utils::{
    edit_file_with_editor, get_directory_location, save_temp_file_contents, set_up_auto_save,
    stop_auto_save,
};

pub fn new(core: &Core, lb_path: &str) -> Result<(), CliError> {
    core.get_account()?;

    let file_metadata = core.create_at_path(lb_path).map_err(|err| match err {
        LbError::UiError(err) => match err {
            CreateFileAtPathError::NoAccount => CliError::no_account(),
            CreateFileAtPathError::NoRoot => CliError::no_root(),
            CreateFileAtPathError::FileAlreadyExists => CliError::file_exists(lb_path),
            CreateFileAtPathError::PathContainsEmptyFile => CliError::path_has_empty_file(lb_path),
            CreateFileAtPathError::PathDoesntStartWithRoot => CliError::path_no_root(lb_path),
            CreateFileAtPathError::DocumentTreatedAsFolder => CliError::doc_treated_as_dir(lb_path),
        },
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    let file_location = format!("{}/{}", get_directory_location()?, file_metadata.decrypted_name);
    let temp_file_path = Path::new(&file_location);
    let _ = File::create(&temp_file_path).map_err(|err| {
        CliError::unexpected(format!("couldn't open temporary file for writing: {:#?}", err))
    })?;

    if file_metadata.file_type == Folder {
        println!("Folder created.");
        return Ok(());
    }

    let watcher = set_up_auto_save(core, file_metadata.id, file_location.clone());

    let edit_was_successful = edit_file_with_editor(&file_location);

    if let Some(ok) = watcher {
        stop_auto_save(ok, file_location.clone());
    }

    if edit_was_successful {
        match save_temp_file_contents(core, file_metadata.id, &file_location) {
            Ok(_) => println!("Document encrypted and saved. Cleaning up temporary file."),
            Err(err) => err.print(),
        }
    } else {
        eprintln!("Your editor indicated a problem, aborting and cleaning up");
        core.delete_file(file_metadata.id)
            .map_err(|err| match err {
                LbError::UiError(err) => match err {
                    FileDeleteError::FileDoesNotExist => CliError::file_not_found(lb_path),
                    FileDeleteError::CannotDeleteRoot => CliError::no_root_ops("delete"),
                },
                LbError::Unexpected(msg) => CliError::unexpected(msg),
            })?;
    }

    fs::remove_file(&temp_file_path).map_err(|err| {
        CliError::unexpected(format!("deleting temporary file '{}': {}", &file_location, err))
    })
}
