use lockbook_core::{Core, Error, GetFileByPathError};
use lockbook_core::{Filter, Uuid};

use crate::error::CliError;
use crate::utils::print_last_successful_sync;

pub fn list(
    core: &Core, ids: bool, documents: bool, folders: bool, all: bool,
) -> Result<(), CliError> {
    core.get_account()?;

    let file_filter = if documents {
        Some(Filter::DocumentsOnly)
    } else if folders {
        Some(Filter::FoldersOnly)
    } else if all {
        None
    } else {
        Some(Filter::LeafNodesOnly)
    };

    let paths = core.list_paths(file_filter)?;

    for path in paths {
        if ids {
            println!("{}: {}", get_by_path(&core, &path)?, path)
        } else {
            println!("{}", path)
        }
    }

    print_last_successful_sync(core)
}

fn get_by_path(core: &Core, path: &str) -> Result<Uuid, CliError> {
    let meta = core.get_by_path(&path).map_err(|err| match err {
        Error::Unexpected(msg) => CliError::unexpected(msg),
        Error::UiError(GetFileByPathError::NoFileAtThatPath) => {
            CliError::unexpected(format!("could not find metadata for path: {path}"))
        }
    })?;

    Ok(meta.id)
}
