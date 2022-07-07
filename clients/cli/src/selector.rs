use crate::utils::get_by_path;
use crate::CliError;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{FuzzySelect, Input};
use lockbook_core::Filter::{DocumentsOnly, FoldersOnly};
use lockbook_core::{
    Core, CreateFileAtPathError, CreateFileError, DecryptedFileMetadata, FileType,
    GetFileByPathError, Uuid,
};
use lockbook_core::{Error as LbError, GetFileByIdError};

/// Select a metadata out of core, can provide a path, or an id, or neither, but but not both
/// if neither are provided it will check if this is an interactive session and launch a fuzzy search
/// it will determine prompt for this search based on the optional target file_type passed. This prompt
/// can also optionally be overridden.
pub fn select_meta(
    core: &Core, path: Option<String>, id: Option<Uuid>, target_file_type: Option<FileType>,
    prompt: Option<&str>,
) -> Result<DecryptedFileMetadata, CliError> {
    let prompt = prompt.unwrap_or(match target_file_type {
        Some(FileType::Document) => "Select a document",
        Some(FileType::Folder) => "Select a folder",
        None => "Select a file",
    });

    let filter = target_file_type.map(|file_type| match file_type {
        FileType::Document => DocumentsOnly,
        FileType::Folder => FoldersOnly,
    });

    match (path, id) {
        // Process the Path provided
        (Some(path), None) => core.get_by_path(&path).map_err(|err| match err {
            LbError::UiError(GetFileByPathError::NoFileAtThatPath) => {
                CliError::file_not_found(&path)
            }
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        }),

        // Process the uuid provided
        (None, Some(id)) => core.get_file_by_id(id).map_err(|err| match err {
            LbError::UiError(GetFileByIdError::NoFileWithThatId) => CliError::file_id_not_found(id),
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        }),

        // Reject if both are provided
        (Some(_), Some(_)) => {
            Err(CliError::input("Provided both a path and an ID, only one is needed!"))
        }

        // If nothing is provided and we can go interactive, launch a fzf, otherwise reject
        (None, None) => {
            if atty::is(atty::Stream::Stdout) {
                let docs = core.list_paths(filter)?;
                let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
                    .with_prompt(prompt)
                    .default(0)
                    .items(&docs)
                    .interact()
                    .unwrap();
                get_by_path(core, &docs[selection])
            } else {
                Err(CliError::input("Either a path or an id is required!"))
            }
        }
    }
}

/// Takes either an optional path, or {a parent + a name}, or neither, but not both.
/// if neither are provided and we are interactive, we will launch an fzf selector
/// If the name ends with a `/` it is assumed to be a folder. Otherwise it is a document.
pub fn create_meta(
    core: &Core, lb_path: Option<String>, parent: Option<Uuid>, name: Option<String>,
) -> Result<DecryptedFileMetadata, CliError> {
    match (lb_path, parent, name) {
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
                let dirs = core.list_paths(Some(FoldersOnly))?;
                let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
                    .with_prompt("Select a parent directory")
                    .default(0)
                    .items(&dirs)
                    .interact()
                    .unwrap();
                let parent = get_by_path(core, &dirs[selection])?.id;

                let name: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Choose a file name:")
                    .interact_text()
                    .map_err(CliError::unexpected)?;

                create_file(core, &name, parent)
            } else {
                Err(CliError::input("Must provide either a path or a parent & name"))
            }
        }

        // Reject invalid combinations of input
        _ => Err(CliError::input("Must provide either a path or a parent & name")),
    }
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
