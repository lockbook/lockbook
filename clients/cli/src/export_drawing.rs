use crate::error::CliResult;
use crate::utils::{get_config, get_image_format};
use crate::{err, err_unexpected};
use lockbook_core::{get_file_by_path, Error as CoreError, GetFileByPathError};
use std::io::{stdout, Write};

pub fn export_drawing(drawing: &str, format: &str) -> CliResult<()> {
    let file_metadata = get_file_by_path(&get_config(), drawing).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(drawing.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let lockbook_format = get_image_format(format);

    stdout().write_all(lockbook_core::export_drawing(
        &get_config(),
        file_metadata.id,
        lockbook_format,
    ).unwrap().as_slice()).unwrap();

    Ok(())
}
