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

use crate::error::CliResult;
use crate::utils::{get_account_or_exit, get_config};
use crate::{err, err_unexpected, path_string};

pub fn backup() -> CliResult<()> {
    get_account_or_exit();

    let leaf_nodes = list_paths(&get_config(), Some(LeafNodesOnly)).map_err(|err| match err {
        CoreError::UiError(ListPathsError::Stub) => err_unexpected!("impossible"),
        CoreError::Unexpected(msg) => err_unexpected!("listing leaf nodes: {}", msg),
    })?;

    let docs = list_paths(&get_config(), Some(DocumentsOnly)).map_err(|err| match err {
        CoreError::UiError(ListPathsError::Stub) => err_unexpected!("impossible"),
        CoreError::Unexpected(msg) => err_unexpected!("listing documents: {}", msg),
    })?;

    let folders = list_paths(&get_config(), Some(FoldersOnly)).map_err(|err| match err {
        CoreError::UiError(ListPathsError::Stub) => err_unexpected!("impossible"),
        CoreError::Unexpected(msg) => err_unexpected!("listing folders: {}", msg),
    })?;

    println!(
        "Creating an index to keep track of {} files",
        leaf_nodes.len()
    );

    let now: DateTime<Utc> = Utc::now();

    let backup_directory = match env::current_dir() {
        Ok(mut path) => {
            path.push(format!("LOCKBOOK_BACKUP_{}", now.format("%Y-%m-%d")));
            path
        }
        Err(err) => return Err(err!(OsPwdMissing(err))),
    };

    fs::create_dir(&backup_directory)
        .map_err(|err| err!(OsCouldNotCreateDir(path_string!(backup_directory), err)))?;

    let index_file_content = leaf_nodes.join("\n");
    let index_path = {
        let mut dir = backup_directory.clone();
        dir.push("lockbook.index");
        dir
    };

    File::create(&index_path)
        .map_err(|err| err!(OsCouldNotCreateFile(path_string!(index_path), err)))?
        .write_all(index_file_content.as_bytes())
        .map_err(|err| err!(OsCouldNotWriteFile(path_string!(index_path), err)))?;

    println!("Creating {} folders", folders.len());
    for folder in folders {
        let path = backup_directory.join(PathBuf::from(folder));
        fs::create_dir_all(&path)
            .map_err(|err| err!(OsCouldNotCreateDir(path_string!(path), err)))?;
    }

    println!("Writing {} documents", docs.len());
    for doc in docs {
        let path = backup_directory.join(PathBuf::from(&doc));

        let mut document = File::create(&path)
            .map_err(|err| err!(OsCouldNotCreateFile(path_string!(path), err)))?;

        let document_metadata = get_file_by_path(&get_config(), &doc).map_err(|err| match err {
            CoreError::UiError(GetFileByPathError::NoFileAtThatPath) | CoreError::Unexpected(_) => {
                err_unexpected!("couldn't get file metadata for: {} error: {:?}", &doc, err)
            }
        })?;

        let document_content =
            read_document(&get_config(), document_metadata.id).map_err(|err| match err {
                CoreError::UiError(ReadDocumentError::TreatedFolderAsDocument)
                | CoreError::UiError(ReadDocumentError::NoAccount)
                | CoreError::UiError(ReadDocumentError::FileDoesNotExist)
                | CoreError::Unexpected(_) => {
                    err_unexpected!("couldn't read file: {} error: {:?}", &doc, err)
                }
            })?;

        document
            .write_all(&document_content)
            .map_err(|err| err!(OsCouldNotWriteFile(doc, err)))?;
    }

    Ok(())
}
