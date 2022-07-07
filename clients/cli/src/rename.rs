use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_core::RenameFileError;
use lockbook_core::Uuid;

use crate::error::CliError;
use crate::selector::select_meta;

pub fn rename(
    core: &Core, path: Option<String>, id: Option<Uuid>, new_name: Option<String>,
) -> Result<(), CliError> {
    core.get_account()?;

    let target_id = select_meta(core, path, id, None, Some("Select a file to rename"))?.id;
    let target_path = core.get_path_by_id(target_id)?;

    let new_name = match new_name {
        Some(new_name) => Ok(new_name),
        None => {
            if atty::is(atty::Stream::Stdout) {
                Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Choose a new name:")
                    .interact_text()
                    .map_err(CliError::unexpected)
            } else {
                Err(CliError::input("Must provide a new name"))
            }
        }
    }?;

    core.rename_file(target_id, &new_name)
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                RenameFileError::NewNameEmpty => CliError::file_name_empty(),
                RenameFileError::CannotRenameRoot => CliError::no_root_ops("rename"),
                RenameFileError::NewNameContainsSlash => CliError::file_name_has_slash(new_name),
                RenameFileError::FileNameNotAvailable => CliError::file_name_taken(new_name),
                RenameFileError::FileDoesNotExist => CliError::file_not_found(target_path),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })
}
