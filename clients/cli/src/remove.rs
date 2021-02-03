use std::io;
use std::io::Write;

use lockbook_core::model::file_metadata::FileType;
use lockbook_core::{
    delete_file, get_and_get_children_recursively, get_file_by_path, Error::UiError,
    Error::Unexpected as UnexpectedError, FileDeleteError, GetAndGetChildrenError,
    GetFileByPathError,
};

use crate::utils::{exit_success, get_account_or_exit, get_config};
use crate::{err_unexpected, exitlb};

pub fn remove(path: &str, force: bool) {
    get_account_or_exit();
    let config = get_config();

    let meta = match get_file_by_path(&config, path) {
        Ok(meta) => meta,
        Err(err) => match err {
            UiError(GetFileByPathError::NoFileAtThatPath) => {
                exitlb!(FileNotFound(path.to_string()))
            }
            UnexpectedError(msg) => err_unexpected!("{}", msg).exit(),
        },
    };

    if meta.file_type == FileType::Folder && !force {
        match get_and_get_children_recursively(&config, meta.id) {
            Ok(children) => {
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
            Err(err) => match err {
                UiError(GetAndGetChildrenError::DocumentTreatedAsFolder) => {
                    exitlb!(DocTreatedAsFolder(path.to_string()))
                }
                UiError(GetAndGetChildrenError::FileDoesNotExist) => {
                    exitlb!(FileNotFound(path.to_string()))
                }
                UnexpectedError(msg) => err_unexpected!("{}", msg).exit(),
            },
        };
    }

    match delete_file(&config, meta.id) {
        Ok(_) => {}
        Err(err) => match err {
            UiError(FileDeleteError::FileDoesNotExist) => exitlb!(FileNotFound(path.to_string())),
            UiError(FileDeleteError::CannotDeleteRoot) => {
                exitlb!(CannotDeleteRoot(path.to_string()))
            }
            UnexpectedError(msg) => err_unexpected!("{}", msg).exit(),
        },
    }
}
