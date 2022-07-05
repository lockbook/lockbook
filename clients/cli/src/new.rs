use dialoguer::theme::ColorfulTheme;
use dialoguer::{FuzzySelect, Input};
use std::fs;

use lockbook_core::{Core, Uuid};
use lockbook_core::{CreateFileAtPathError, FileType};
use lockbook_core::{CreateFileError, Error as LbError};
use lockbook_core::{DecryptedFileMetadata, FileMetadata};
use lockbook_core::{FileDeleteError, Filter, GetFileByPathError};

use crate::error::CliError;
use crate::utils::{
    edit_file_with_editor, get_directory_location, save_temp_file_contents, set_up_auto_save,
    stop_auto_save,
};

pub fn new(
    core: &Core, lb_path: Option<String>, parent: Option<Uuid>, name: Option<String>,
) -> Result<(), CliError> {
    core.get_account()?;

    let file_metadata = match (lb_path, parent, name) {
        // Create a new file at the given path
        (Some(path), None, None) => core.create_at_path(&path).map_err(|err| match err {
            LbError::UiError(err) => match err {
                CreateFileAtPathError::NoRoot => CliError::no_root(),
                CreateFileAtPathError::FileAlreadyExists => CliError::file_exists(path),
                CreateFileAtPathError::PathContainsEmptyFile => CliError::path_has_empty_file(path),
                CreateFileAtPathError::DocumentTreatedAsFolder => {
                    CliError::doc_treated_as_dir(path)
                }
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        }),

        // Create a file with the specified parent and name
        (None, Some(parent), Some(name)) => create_file(core, &name, parent),

        // If we can, interactively create the desired file
        (None, None, None) => {
            if atty::is(atty::Stream::Stdout) {
                let dirs = core.list_paths(Some(Filter::FoldersOnly))?;
                let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
                    .with_prompt("Select a parent directory")
                    .default(0)
                    .items(&dirs)
                    .interact()
                    .unwrap();
                let parent = core
                    .get_by_path(&dirs[selection])
                    .map_err(|err| match err {
                        LbError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                            CliError::file_not_found(&dirs[selection])
                        }
                        LbError::Unexpected(msg) => CliError::unexpected(msg),
                    })?
                    .id;

                let name: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Choose a file name:")
                    .interact_text()
                    .map_err(|err| CliError::unexpected(err))?;

                create_file(core, &name, parent)
            } else {
                Err(CliError::input("Must provide either a path or a parent & name"))
            }
        }

        // Reject invalid combinations of input
        _ => Err(CliError::input("Must provide either a path or a parent & name")),
    }?;

    let mut temp_file_path = get_directory_location()?;
    temp_file_path.push(&file_metadata.decrypted_name);
    let _ = fs::File::create(&temp_file_path).map_err(|err| {
        CliError::unexpected(format!("couldn't open temporary file for writing: {:#?}", err))
    })?;

    if file_metadata.is_folder() {
        println!("Folder created.");
        return Ok(());
    }

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
        let path = core.get_path_by_id(file_metadata.id)?;
        core.delete_file(file_metadata.id)
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

fn create_file(core: &Core, name: &str, parent: Uuid) -> Result<DecryptedFileMetadata, CliError> {
    let file_type = if name.ends_with('/') { FileType::Folder } else { FileType::Document };
    let name =
        if name.ends_with('/') { name[0..name.len() - 1].to_string() } else { name.to_string() };
    let parent_path = core.get_path_by_id(parent)?;
    core.create_file(&name, parent, file_type)
        .map_err(|err| match err {
            LbError::UiError(CreateFileError::DocumentTreatedAsFolder) => {
                CliError::doc_treated_as_dir(parent_path)
            }
            LbError::UiError(CreateFileError::CouldNotFindAParent) => {
                CliError::file_not_found(parent_path)
            }
            LbError::UiError(CreateFileError::FileNameEmpty) => CliError::file_name_empty(),
            LbError::UiError(CreateFileError::FileNameContainsSlash) => {
                CliError::file_name_has_slash(name)
            }
            LbError::UiError(CreateFileError::FileNameNotAvailable) => {
                CliError::file_name_taken(name)
            }
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })
}
