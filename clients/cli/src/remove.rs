use std::io;
use std::io::Write;

use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_core::FileDeleteError;
use lockbook_core::FileMetadata;
use lockbook_core::GetAndGetChildrenError;
use lockbook_core::Uuid;

use crate::error::CliError;
use crate::selector::select_meta;

pub fn remove(
    core: &Core, lb_path: Option<String>, id: Option<Uuid>, force: bool,
) -> Result<(), CliError> {
    core.get_account()?;

    let meta = select_meta(core, lb_path, id, None, Some("Select a file to delete"))?;
    let path = &core.get_path_by_id(meta.id)?;

    if meta.is_folder() && !force {
        let children = core
            .get_and_get_children_recursively(meta.id)
            .map_err(|err| match err {
                LbError::UiError(err) => match err {
                    GetAndGetChildrenError::DocumentTreatedAsFolder => {
                        CliError::doc_treated_as_dir(path)
                    }
                    GetAndGetChildrenError::FileDoesNotExist => CliError::file_not_found(path),
                },
                LbError::Unexpected(msg) => CliError::unexpected(msg),
            })?;

        print!(
            "Are you sure you want to delete {} documents? [y/n]: ",
            children
                .into_iter()
                .filter(|child| child.is_document())
                .count()
        );
        io::stdout().flush()?;

        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .expect("Failed to read from stdin");
        answer.retain(|c| c != '\n' && c != '\r');

        if answer != "y" && answer != "Y" {
            println!("Aborted.");
            return Ok(());
        }
    }

    core.delete_file(meta.id).map_err(|err| match err {
        LbError::UiError(FileDeleteError::FileDoesNotExist) => CliError::file_not_found(path),
        LbError::UiError(FileDeleteError::CannotDeleteRoot) => CliError::no_root_ops("delete"),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })
}
