use std::io;
use std::io::Write;

use lockbook_core::Error as LbError;
use lockbook_core::ExportDrawingError;
use lockbook_core::SupportedImageFormats;
use lockbook_core::{Core, Uuid};

use crate::error::CliError;
use crate::selector::select_meta;

pub fn drawing(
    core: &Core, lb_path: Option<String>, id: Option<Uuid>, format: &str,
) -> Result<(), CliError> {
    let file_metadata = select_meta(core, lb_path, id, None, None)?;
    let lb_path = core.get_path_by_id(file_metadata.id)?;

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
                ExportDrawingError::FolderTreatedAsDrawing => {
                    CliError::dir_treated_as_doc(&file_metadata)
                }
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
