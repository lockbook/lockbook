use std::io;
use std::io::Write;

use lockbook_core::model::errors::GetFileByPathError;
use lockbook_core::Core;
use lockbook_core::Error as LbError;

use crate::error::CliError;

pub fn print(core: &Core, lb_path: &str) -> Result<(), CliError> {
    core.get_account()?;

    let file_metadata = core.get_by_path(lb_path).map_err(|err| match err {
        LbError::UiError(GetFileByPathError::NoFileAtThatPath) => CliError::file_not_found(lb_path),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    let content = core
        .read_document(file_metadata.id)
        .map_err(|err| CliError::unexpected(format!("{:?}", err)))?;
    print!("{}", String::from_utf8_lossy(&content));

    io::stdout()
        .flush()
        .map_err(|err| CliError::unexpected(format!("flushing stdin: {:#?}", err)))
}
