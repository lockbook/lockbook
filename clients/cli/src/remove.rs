use std::io;
use std::io::Write;

use lockbook_core::model::errors::FileDeleteError;
use lockbook_core::model::errors::GetAndGetChildrenError;
use lockbook_core::model::errors::GetFileByPathError;
use lockbook_core::Error as LbError;
use lockbook_core::LbCore;
use lockbook_models::file_metadata::FileType;

use crate::error::CliError;

pub fn remove(core: &LbCore, lb_path: &str, force: bool) -> Result<(), CliError> {
    core.get_account()?;

    let meta = core.get_by_path(lb_path).map_err(|err| match err {
        LbError::UiError(GetFileByPathError::NoFileAtThatPath) => CliError::file_not_found(lb_path),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    if meta.file_type == FileType::Folder && !force {
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
                .filter(|child| child.file_type == FileType::Document)
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
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })
}
