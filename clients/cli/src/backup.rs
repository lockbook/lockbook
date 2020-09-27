use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{env, fs};

use chrono::{DateTime, Utc};
use lockbook_core::repo::file_metadata_repo::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
use lockbook_core::{get_file_by_path, list_paths, GetFileByPathError, ListPathsError, read_document, ReadDocumentError};

use crate::utils::{exit_with, get_account_or_exit, get_config};
use crate::{
    COULD_NOT_CREATE_OS_DIRECTORY, COULD_NOT_WRITE_TO_OS_FILE, PWD_MISSING, UNEXPECTED_ERROR,
};

pub fn backup() {
    get_account_or_exit();

    let now: DateTime<Utc> = Utc::now();

    let backup_directory = match env::current_dir() {
        Ok(mut path) => {
            path.push(format!("LOCKBOOK_BACKUP_{}", now.format("%Y-%m-%d")));
            path
        }
        Err(err) => exit_with(
            &format!("Could not get PWD from OS, error: {}", err),
            PWD_MISSING,
        ),
    };

    fs::create_dir(&backup_directory).unwrap_or_else(|err| {
        exit_with(
            &format!("Could not create backup directory! Error: {}", err),
            COULD_NOT_CREATE_OS_DIRECTORY,
        )
    });

    let leaf_nodes =
        list_paths(&get_config(), Some(LeafNodesOnly)).unwrap_or_else(|err| match err {
            ListPathsError::UnexpectedError(msg) => exit_with(
                &format!("Unexpected error while listing leaf nodes: {}", msg),
                UNEXPECTED_ERROR,
            ),
        });

    let docs = list_paths(&get_config(), Some(DocumentsOnly)).unwrap_or_else(|err| match err {
        ListPathsError::UnexpectedError(msg) => exit_with(
            &format!("Unexpected error while listing documents: {}", msg),
            UNEXPECTED_ERROR,
        ),
    });

    let folders = list_paths(&get_config(), Some(FoldersOnly)).unwrap_or_else(|err| match err {
        ListPathsError::UnexpectedError(msg) => exit_with(
            &format!("Unexpected error while listing documents: {}", msg),
            UNEXPECTED_ERROR,
        ),
    });

    println!(
        "Creating an index to keep track of {} files",
        leaf_nodes.len()
    );

    let index_file_path = {
        let mut dir = backup_directory.clone();
        dir.push("lockbook.index");
        dir
    };
    let mut index_file = File::create(&index_file_path).unwrap_or_else(|err| {
        exit_with(
            &format!("Could not create index file, error: {}", err),
            COULD_NOT_WRITE_TO_OS_FILE,
        )
    });
    let index_file_content: String = leaf_nodes.join("\n");
    index_file
        .write_all(index_file_content.as_bytes())
        .unwrap_or_else(|err| {
            exit_with(
                &format!("Could not write to index file: {}", err),
                COULD_NOT_WRITE_TO_OS_FILE,
            )
        });

    println!("Creating {} folders", folders.len());
    for folder in folders {
        let path = backup_directory.join(PathBuf::from(folder));
        fs::create_dir_all(&path).unwrap_or_else(|err| {
            exit_with(
                &format!(
                    "Could not create {:?} directory! Error: {}",
                    path.to_str(),
                    err
                ),
                COULD_NOT_CREATE_OS_DIRECTORY,
            )
        });
    }

    println!("Writing {} documents", docs.len());
    for doc in docs {
        let path = backup_directory.join(PathBuf::from(&doc));

        let mut document = File::create(&path).unwrap_or_else(|err| {
            exit_with(
                &format!("Could not create index file, error: {}", err),
                COULD_NOT_WRITE_TO_OS_FILE,
            )
        });

        let document_metadata =
            get_file_by_path(&get_config(), &doc).unwrap_or_else(|err| match err {
                GetFileByPathError::NoFileAtThatPath | GetFileByPathError::UnexpectedError(_) => {
                    exit_with(
                        &format!("Could not get file metadata for: {} error: {:?}", &doc, err),
                        UNEXPECTED_ERROR,
                    )
                }
            });

        let document_content = read_document(&get_config(), document_metadata.id).unwrap_or_else(|err| match err {
            ReadDocumentError::TreatedFolderAsDocument |
            ReadDocumentError::NoAccount |
            ReadDocumentError::FileDoesNotExist |
            ReadDocumentError::UnexpectedError(_) => exit_with(
                &format!("Could not read file: {} error: {:?}", &doc, err),
                UNEXPECTED_ERROR,
            ),
        }).secret;

        document
            .write_all(document_content.as_bytes())
            .unwrap_or_else(|err| {
                exit_with(
                    &format!("Could not write to file: {}, error: {}", &doc, err),
                    COULD_NOT_WRITE_TO_OS_FILE,
                )
            });
    }
}
