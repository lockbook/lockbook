use crate::error::CliResult;
use crate::utils::{config, get_image_format};
use crate::{err, err_unexpected};
use lockbook_core::model::errors::ExportDrawingError;
use lockbook_core::model::errors::GetFileByPathError;
use lockbook_core::{get_file_by_path, Error as CoreError};
use std::io::{stdout, Write};

pub fn export_drawing(lb_path: &str, format: &str) -> CliResult<()> {
    let file_metadata = get_file_by_path(&config()?, lb_path).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(lb_path.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let lockbook_format = get_image_format(format);

    let drawing_bytes =
        lockbook_core::export_drawing(&config()?, file_metadata.id, lockbook_format, None)
            .map_err(|err| match err {
                CoreError::UiError(ui_err) => match ui_err {
                    ExportDrawingError::FolderTreatedAsDrawing => {
                        err!(FolderTreatedAsDoc(lb_path.to_string()))
                    }
                    ExportDrawingError::NoAccount => err!(NoAccount),
                    ExportDrawingError::InvalidDrawing => {
                        err!(InvalidDrawing(file_metadata.decrypted_name))
                    }
                    ExportDrawingError::FileDoesNotExist => {
                        err!(FileNotFound(file_metadata.decrypted_name))
                    }
                },
                CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
            })?;

    stdout()
        .write_all(drawing_bytes.as_slice())
        .map_err(|err| err_unexpected!("{:#?}", err))?;

    Ok(())
}
