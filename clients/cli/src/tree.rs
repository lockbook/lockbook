use lockbook_core::LbCore;
use lockbook_models::tree::FileMetaExt;

use crate::error::CliError;

pub fn tree(core: &LbCore) -> Result<(), CliError> {
    core.get_account()?;

    let files = core
        .list_metadatas()
        .map_err(|err| CliError::unexpected(format!("{}", err)))?;

    println!("{}", files.pretty_print());

    Ok(())
}
