use std::io;
use std::io::Write;

use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_core::FileDeleteError;
use lockbook_core::FileMetadata;
use lockbook_core::GetAndGetChildrenError;
use lockbook_core::GetFileByPathError;

use crate::error::CliError;

pub fn remove(core: &Core, lb_path: &str, force: bool) -> Result<(), CliError> {
    core.get_account()?;

    let meta = core.get_by_path(lb_path).map_err(|err| match err {
        LbError::UiError(GetFileByPathError::NoFileAtThatPath) => CliError::file_not_found(lb_path),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    if meta.is_folder() && !force {
        let children = core
            .get_and_get_children_recursively(meta.id)
            .map_err(|err| match err {
                LbError::UiError(err) => match err {
                    GetAndGetChildrenError::DocumentTreatedAsFolder => {
                        CliError::doc_treated_as_dir(lb_path)
                    }
                    GetAndGetChildrenError::FileDoesNotExist => CliError::file_not_found(lb_path),
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
        io::stdout().flush().unwrap();

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
        LbError::UiError(FileDeleteError::FileDoesNotExist) => CliError::file_not_found(lb_path),
        LbError::UiError(FileDeleteError::CannotDeleteRoot) => CliError::no_root_ops("delete"),
        LbError::UiError(FileDeleteError::InsufficientPermission) => todo!(), // todo(sharing)
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })
}
