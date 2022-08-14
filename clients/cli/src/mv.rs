use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_core::FileType;
use lockbook_core::MoveFileError;
use lockbook_core::Uuid;

use crate::error::CliError;
use crate::selector::select_meta;

pub fn mv(
    core: &Core, src: Option<String>, src_id: Option<Uuid>, dest: Option<String>,
    dest_id: Option<Uuid>,
) -> Result<(), CliError> {
    core.get_account()?;

    let src_meta = select_meta(core, src, src_id, None, Some("Select a file to move"))?;
    let src_path = core.get_path_by_id(src_meta.id)?;

    let dest_meta = select_meta(
        core,
        dest,
        dest_id,
        Some(FileType::Folder),
        Some("Select a target directory"),
    )?;
    let dest_path = core.get_path_by_id(dest_meta.id)?;

    core.move_file(src_meta.id, dest_meta.id)
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                MoveFileError::CannotMoveRoot => CliError::no_root_ops("move"),
                MoveFileError::FileDoesNotExist => CliError::file_not_found(src_path),
                MoveFileError::TargetParentDoesNotExist => CliError::file_not_found(dest_path),
                MoveFileError::FolderMovedIntoItself => CliError::moving_folder_into_itself(),
                MoveFileError::TargetParentHasChildNamedThat => CliError::file_name_taken(""), //todo
                MoveFileError::DocumentTreatedAsFolder => CliError::doc_treated_as_dir(dest_path)
                    .with_extra(format!("{} cannot be moved to {}", src_meta.name, dest_meta.name)),
                MoveFileError::LinkInSharedFolder => CliError::link_in_shared(dest_meta.name),
                MoveFileError::InsufficientPermission => {
                    CliError::no_write_permission(src_meta.name)
                }
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })
}
