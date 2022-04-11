use lockbook_core::list_metadatas;
use lockbook_core::LbCore;
use lockbook_models::tree::FileMetaExt;

use crate::error::CliError;
use crate::utils::config;

pub fn tree(core: &LbCore) -> Result<(), CliError> {
    core.get_account()?;

    let files =
        list_metadatas(&config()?).map_err(|err| CliError::unexpected(format!("{}", err)))?;

    println!("{}", files.pretty_print());

    Ok(())
}
