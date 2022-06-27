use std::io;
use std::io::Write;

use lockbook_core::ExportDrawingError;
use lockbook_core::GetFileByPathError;
use lockbook_core::pure_functions::drawing::SupportedImageFormats;
use lockbook_core::Core;
use lockbook_core::Error as LbError;

use crate::error::CliError;

pub fn export_drawing(core: &Core, lb_path: &str, format: &str) -> Result<(), CliError> {
    let file_metadata = core.get_by_path(lb_path).map_err(|err| match err {
        LbError::UiError(GetFileByPathError::NoFileAtThatPath) => CliError::file_not_found(lb_path),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    let lockbook_format = format.parse().unwrap_or_else(|_| {
        eprintln!(
            "'{}' is an unsupported format, but feel free to make a github issue! Falling back to PNG for now.",
            format
        );
        SupportedImageFormats::Png
    });

    let drawing_bytes = core
        .export_drawing(file_metadata.id, lockbook_format, None)
        .map_err(|err| match err {
            LbError::UiError(err) => match err {
                ExportDrawingError::FolderTreatedAsDrawing => CliError::dir_treated_as_doc(lb_path),
                ExportDrawingError::InvalidDrawing => {
                    CliError::invalid_drawing(file_metadata.decrypted_name)
                }
                ExportDrawingError::FileDoesNotExist => CliError::file_not_found(lb_path),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        })?;

    io::stdout()
        .write_all(drawing_bytes.as_slice())
        .map_err(|err| CliError::unexpected(format!("{:#?}", err)))?;

    Ok(())
}
