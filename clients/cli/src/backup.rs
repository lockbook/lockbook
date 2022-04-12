use std::io::Write;
use std::{env, fs};

use chrono::{DateTime, Utc};

use lockbook_core::model::errors::ExportFileError;
use lockbook_core::model::errors::GetRootError;
use lockbook_core::service::path_service::Filter::{DocumentsOnly, FoldersOnly, LeafNodesOnly};
use lockbook_core::Error as LbError;
use lockbook_core::LbCore;

use crate::error::CliError;

pub fn backup(core: &LbCore) -> Result<(), CliError> {
    core.get_account()?;

    let leaf_nodes = core.list_paths(Some(LeafNodesOnly))?;
    let n_docs = core.list_paths(Some(DocumentsOnly))?.len();
    let n_folders = core.list_paths(Some(FoldersOnly))?.len();

    println!("Creating an index to keep track of {} files", leaf_nodes.len());

    let now: DateTime<Utc> = Utc::now();

    let backup_dir = env::current_dir()
        .map(|mut path| {
            path.push(format!("LOCKBOOK_BACKUP_{}", now.format("%Y-%m-%d")));
            path
        })
        .map_err(CliError::os_current_dir)?;

    fs::create_dir(&backup_dir).map_err(|err| CliError::os_mkdir(&backup_dir, err))?;

    let index_file_content = leaf_nodes.join("\n");
    let index_path = {
        let mut dir = backup_dir.clone();
        dir.push("lockbook.index");
        dir
    };

    fs::File::create(&index_path)
        .map_err(|err| CliError::os_create_file(&index_path, err))?
        .write_all(index_file_content.as_bytes())
        .map_err(|err| CliError::os_write_file(index_path, err))?;

    println!("Backing up {} folders and {} documents.", n_folders, n_docs);

    let root = core.get_root().map_err(|err| match err {
        LbError::UiError(GetRootError::NoRoot) => CliError::no_root(),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    core.export_file(root.id, backup_dir.clone(), false, None)
        .map_err(|err| match err {
            LbError::UiError(ExportFileError::NoAccount) => CliError::no_account(),
            LbError::UiError(ExportFileError::DiskPathTaken) => {
                CliError::os_file_collision(backup_dir)
            }
            LbError::UiError(ExportFileError::DiskPathInvalid) => {
                CliError::os_invalid_path(backup_dir)
            }
            LbError::UiError(ExportFileError::ParentDoesNotExist) | LbError::Unexpected(_) => {
                CliError::unexpected(format!("{:#?}", err))
            }
        })
}
