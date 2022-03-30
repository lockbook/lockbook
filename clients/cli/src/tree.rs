use crate::error::CliResult;
use crate::utils::metadatas;
use lockbook_models::tree::{FileMetaExt, TreeError};
use crate::{err, err_unexpected};

pub fn tree() -> CliResult<()> {
    let files = metadatas()?;

    match files.display() {
        Ok(tree) => {
            println!("\n{}", tree);
            return Ok(())
        },
        Err(err) => return match err {
            TreeError::RootNonexistent => Err(err!(NoRoot)),
            TreeError::Unexpected(msg) => Err(err_unexpected!("{}", msg)),
            _ => Err(err_unexpected!("{}", "Failed to display file tree.")),
        },
    }
}