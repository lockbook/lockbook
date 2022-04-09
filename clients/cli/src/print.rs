use crate::error::CliResult;
use crate::utils::{account, config};
use crate::{err, err_unexpected};
use lockbook_core::model::errors::GetFileByPathError;
use lockbook_core::{get_file_by_path, read_document, Error as CoreError};
use std::io;
use std::io::Write;

pub fn print(file_name: &str) -> CliResult<()> {
    account()?;
    let cfg = config()?;

    let file_metadata = get_file_by_path(&cfg, file_name).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(file_name.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let content =
        read_document(&cfg, file_metadata.id).map_err(|err| err_unexpected!("{:?}", err))?;
    print!("{}", String::from_utf8_lossy(&content));

    io::stdout()
        .flush()
        .map_err(|err| err_unexpected!("flushing stdin: {:#?}", err))
}
