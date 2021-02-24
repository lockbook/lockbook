use crate::error::CliResult;
use crate::utils::{get_config, get_image_format};
use crate::{err, err_unexpected};
use lockbook_core::{get_file_by_path, Error as CoreError, ExportDrawingError, GetFileByPathError};
use std::io::{stdout, Write};

pub fn export_drawing(drawing: &str, format: &str) -> CliResult<()> {
    let file_metadata = get_file_by_path(&get_config(), drawing).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(drawing.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let lockbook_format = get_image_format(format);

    let drawing_bytes =
        lockbook_core::export_drawing(&get_config(), file_metadata.id, lockbook_format).map_err(
            |err| match err {
                CoreError::UiError(ui_err) => match ui_err {
                    ExportDrawingError::FolderTreatedAsDrawing => {
                        err!(FolderTreatedAsDoc(drawing.to_string()))
                    }
                    ExportDrawingError::NoAccount => err!(NoAccount),
                    ExportDrawingError::InvalidDrawing => err!(InvalidDrawing(file_metadata.name)),
                    ExportDrawingError::FileDoesNotExist => err!(FileNotFound(file_metadata.name)),
                },
                CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
            },
        )?;

    stdout()
        .write_all(drawing_bytes.as_slice())
        .map_err(|err| err_unexpected!("{:#?}", err))?;

    Ok(())
}
