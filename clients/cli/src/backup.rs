use std::fs::File;
use std::io::Write;
use std::{env, fs};

use chrono::{DateTime, Utc};

use lockbook_core::service::path_service::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
use lockbook_core::{
    export_file, get_root, list_paths, Error as CoreError, ExportFileError, GetRootError,
};

use crate::error::CliResult;
use crate::utils::{account, config};
use crate::{err, err_unexpected, path_string};

pub fn backup() -> CliResult<()> {
    account()?;

    let config = config()?;

    let leaf_nodes = list_paths(&config, Some(LeafNodesOnly))?;

    let docs_len = list_paths(&config, Some(DocumentsOnly))?.len();

    let folders_len = list_paths(&config, Some(FoldersOnly))?.len();

    println!("Creating an index to keep track of {} files", leaf_nodes.len());

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

    println!("Backing up {} folders and {} documents.", folders_len, docs_len);

    let root = get_root(&config).map_err(|err| match err {
        CoreError::UiError(GetRootError::NoRoot) => err!(NoRoot),
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    export_file(&config, root.id, backup_directory.clone(), false, None).map_err(|err| match err {
        CoreError::UiError(ExportFileError::NoAccount) => err!(NoAccount),
        CoreError::UiError(ExportFileError::DiskPathTaken) => {
            err!(OsFileCollision(format!("{}", backup_directory.display())))
        }
        CoreError::UiError(ExportFileError::DiskPathInvalid) => {
            err!(OsInvalidPath(format!("{}", backup_directory.display())))
        }
        CoreError::UiError(ExportFileError::ParentDoesNotExist) | CoreError::Unexpected(_) => {
            err_unexpected!("{:#?}", err)
        }
    })
}
