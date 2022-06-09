use lockbook_core::Core;
use lockbook_models::tree::{FileMetaMapExt, FileMetaVecExt};

use crate::error::CliError;

pub fn tree(core: &Core) -> Result<(), CliError> {
    core.get_account()?;

    let files = core
        .list_metadatas()
        .map_err(|err| CliError::unexpected(format!("{}", err)))?;

    println!("{}", files.to_map().pretty_print());

    Ok(())
}
