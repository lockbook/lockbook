use std::io;
use std::io::Write;

use lockbook_core::model::errors::ExportDrawingError;
use lockbook_core::model::errors::GetFileByPathError;
use lockbook_core::Error as LbError;
use lockbook_core::LbCore;

use crate::error::CliError;
use crate::utils::{config, get_image_format};

pub fn export_drawing(core: &LbCore, lb_path: &str, format: &str) -> Result<(), CliError> {
    let file_metadata = core.get_by_path(lb_path).map_err(|err| match err {
        LbError::UiError(GetFileByPathError::NoFileAtThatPath) => CliError::file_not_found(lb_path),
        LbError::Unexpected(msg) => CliError::unexpected(msg),
    })?;

    let lockbook_format = get_image_format(format);

    let drawing_bytes =
        lockbook_core::export_drawing(&config()?, file_metadata.id, lockbook_format, None)
            .map_err(|err| match err {
                LbError::UiError(err) => match err {
                    ExportDrawingError::FolderTreatedAsDrawing => {
                        CliError::dir_treated_as_doc(lb_path)
                    }
                    ExportDrawingError::NoAccount => CliError::no_account(),
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
