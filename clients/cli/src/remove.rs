use std::io;
use std::io::Write;

use lockbook_core::model::file_metadata::FileType;
use lockbook_core::{
    delete_file, get_and_get_children_recursively, get_file_by_path, Error, Error::UiError,
    Error::Unexpected as UnexpectedError, FileDeleteError, GetAndGetChildrenError,
    GetFileByPathError,
};

use crate::utils::{exit_with, get_account_or_exit, get_config};
use crate::{COULD_NOT_DELETE_ROOT, DOCUMENT_TREATED_AS_FOLDER, FILE_NOT_FOUND, UNEXPECTED_ERROR};

pub fn remove(path: &str) {
    get_account_or_exit();
    let config = get_config();

    let meta = match get_file_by_path(&config, path) {
        Ok(meta) => meta,
        Err(err) => match err {
            UiError(GetFileByPathError::NoFileAtThatPath) => exit_with(
                &format!("No file found with the path {}", path),
                FILE_NOT_FOUND,
            ),
            UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    };

    if meta.file_type == FileType::Folder {
        match get_and_get_children_recursively(&config, meta.id) {
            Ok(children) => {
                print!(
                    "Are you sure you want to delete {} files? [y/n]: ",
                    children.len()
                );
                io::stdout().flush().unwrap();

                let mut answer = String::new();
                io::stdin()
                    .read_line(&mut answer)
                    .expect("Failed to read from stdin");
                answer.retain(|c| c != '\n');

                if answer != "y" && answer != "Y" {
                    exit_with("Aborted.", 0)
                }
            }
            Err(err) => match err {
                UiError(GetAndGetChildrenError::DocumentTreatedAsFolder) => exit_with(
                    &format!("File {} is a document", path),
                    DOCUMENT_TREATED_AS_FOLDER,
                ),
                UiError(GetAndGetChildrenError::FileDoesNotExist) => exit_with(
                    &format!("No file found with the path {}", path),
                    FILE_NOT_FOUND,
                ),
                UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            },
        };
    }

    match delete_file(&config, meta.id) {
        Ok(_) => {}
        Err(err) => match err {
            UiError(FileDeleteError::FileDoesNotExist) => exit_with(
                &format!("Cannot delete '{}', file does not exist.", path),
                FILE_NOT_FOUND,
            ),
            UiError(FileDeleteError::CannotDeleteRoot) => exit_with(
                &format!("Cannot delete '{}' since it is the root folder.", path),
                COULD_NOT_DELETE_ROOT,
            ),
            UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }
}
