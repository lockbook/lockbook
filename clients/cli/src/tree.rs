use crate::error::CliResult;
use crate::utils::metadatas;
use lockbook_models::tree::FileMetaExt;

pub fn tree() -> CliResult<()> {
    let files = metadatas()?;

    println!("{}", files.pretty_print());

    Ok(())
}
