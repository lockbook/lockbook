use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{env, fs};

use chrono::{DateTime, Utc};

use lockbook_core::repo::file_metadata_repo::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
use lockbook_core::{
    get_file_by_path, list_paths, read_document, Error as CoreError, GetFileByPathError,
    ListPathsError, ReadDocumentError,
};

use crate::utils::{get_account_or_exit, get_config};
use crate::{err_unexpected, exitlb, pathbuf_string};

pub fn backup() {
    get_account_or_exit();

    let now: DateTime<Utc> = Utc::now();

    let backup_directory = match env::current_dir() {
        Ok(mut path) => {
            path.push(format!("LOCKBOOK_BACKUP_{}", now.format("%Y-%m-%d")));
            path
        }
        Err(err) => exitlb!(PwdMissing(err)),
    };

    fs::create_dir(&backup_directory).unwrap_or_else(|err| {
        exitlb!(
            OsCouldNotCreateDir,
            "Could not create backup directory! Error: {}",
            err
        )
    });

    let leaf_nodes =
        list_paths(&get_config(), Some(LeafNodesOnly)).unwrap_or_else(|err| match err {
            CoreError::UiError(ListPathsError::Stub) => err_unexpected!("impossible").exit(),
            CoreError::Unexpected(msg) => err_unexpected!("listing leaf nodes: {}", msg).exit(),
        });

    let docs = list_paths(&get_config(), Some(DocumentsOnly)).unwrap_or_else(|err| match err {
        CoreError::UiError(ListPathsError::Stub) => err_unexpected!("Impossible").exit(),
        CoreError::Unexpected(msg) => err_unexpected!("listing documents: {}", msg).exit(),
    });

    let folders = list_paths(&get_config(), Some(FoldersOnly)).unwrap_or_else(|err| match err {
        CoreError::UiError(ListPathsError::Stub) => err_unexpected!("impossible").exit(),
        CoreError::Unexpected(msg) => err_unexpected!("listing folders: {}", msg).exit(),
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
    let mut index_file = File::create(&index_file_path)
        .unwrap_or_else(|err| exitlb!(OsCouldNotCreateFile(pathbuf_string!(index_file_path), err)));

    let index_file_content: String = leaf_nodes.join("\n");
    index_file
        .write_all(index_file_content.as_bytes())
        .unwrap_or_else(|err| exitlb!(OsCouldNotWriteFile(pathbuf_string!(index_file_path), err)));

    println!("Creating {} folders", folders.len());
    for folder in folders {
        let path = backup_directory.join(PathBuf::from(folder));
        fs::create_dir_all(&path).unwrap_or_else(|err| {
            exitlb!(
                OsCouldNotCreateDir,
                "Could not create {:?} directory! Error: {}",
                path,
                err
            )
        });
    }

    println!("Writing {} documents", docs.len());
    for doc in docs {
        let path = backup_directory.join(PathBuf::from(&doc));

        let mut document = File::create(&path)
            .unwrap_or_else(|err| exitlb!(OsCouldNotCreateFile(pathbuf_string!(path), err)));

        let document_metadata =
            get_file_by_path(&get_config(), &doc).unwrap_or_else(|err| match err {
                CoreError::UiError(GetFileByPathError::NoFileAtThatPath)
                | CoreError::Unexpected(_) => {
                    err_unexpected!("couldn't get file metadata for: {} error: {:?}", &doc, err)
                        .exit()
                }
            });

        let document_content =
            read_document(&get_config(), document_metadata.id).unwrap_or_else(|err| match err {
                CoreError::UiError(ReadDocumentError::TreatedFolderAsDocument)
                | CoreError::UiError(ReadDocumentError::NoAccount)
                | CoreError::UiError(ReadDocumentError::FileDoesNotExist)
                | CoreError::Unexpected(_) => {
                    err_unexpected!("couldn't read file: {} error: {:?}", &doc, err).exit()
                }
            });

        document
            .write_all(&document_content)
            .unwrap_or_else(|err| exitlb!(OsCouldNotWriteFile(doc, err)));
    }
}
