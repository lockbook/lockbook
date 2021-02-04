use std::io;
use std::io::Write;

use lockbook_core::model::file_metadata::FileType;
use lockbook_core::{
    delete_file, get_and_get_children_recursively, get_file_by_path, Error::UiError,
    Error::Unexpected as UnexpectedError, FileDeleteError, GetAndGetChildrenError,
    GetFileByPathError,
};

use crate::error::CliResult;
use crate::utils::{exit_success, get_account_or_exit, get_config};
use crate::{err, err_unexpected};

pub fn remove(path: &str, force: bool) -> CliResult<()> {
    get_account_or_exit();
    let config = get_config();

    let meta = get_file_by_path(&config, path).map_err(|err| match err {
        UiError(GetFileByPathError::NoFileAtThatPath) => err!(FileNotFound(path.to_string())),
        UnexpectedError(msg) => err_unexpected!("{}", msg),
    })?;

    if meta.file_type == FileType::Folder && !force {
        let children =
            get_and_get_children_recursively(&config, meta.id).map_err(|err| match err {
                UiError(GetAndGetChildrenError::DocumentTreatedAsFolder) => {
                    err!(DocTreatedAsFolder(path.to_string()))
                }
                UiError(GetAndGetChildrenError::FileDoesNotExist) => {
                    err!(FileNotFound(path.to_string()))
                }
                UnexpectedError(msg) => err_unexpected!("{}", msg).exit(),
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
        answer.retain(|c| c != '\n');

        if answer != "y" && answer != "Y" {
            exit_success("Aborted.")
        }
    }

    delete_file(&config, meta.id).map_err(|err| match err {
        UiError(FileDeleteError::FileDoesNotExist) => err!(FileNotFound(path.to_string())),
        UiError(FileDeleteError::CannotDeleteRoot) => err!(CannotDeleteRoot(path.to_string())),
        UnexpectedError(msg) => err_unexpected!("{}", msg),
    })
}
