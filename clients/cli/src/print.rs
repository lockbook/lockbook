use std::io;
use std::io::Write;

use lockbook_core::Core;
use lockbook_core::FileType::Document;
use lockbook_core::Uuid;

use crate::error::CliError;
use crate::selector::select_meta;

pub fn print(core: &Core, lb_path: Option<String>, id: Option<Uuid>) -> Result<(), CliError> {
    core.get_account()?;

    let file_metadata = select_meta(core, lb_path, id, Some(Document), None)?;

    let content = core
        .read_document(file_metadata.id)
        .map_err(|err| CliError::unexpected(format!("{:?}", err)))?;
    print!("{}", String::from_utf8_lossy(&content));

    io::stdout()
        .flush()
        .map_err(|err| CliError::unexpected(format!("flushing stdin: {:#?}", err)))
}
