use crate::error::CliResult;
use crate::utils::{get_config, get_image_format};
use crate::{err, err_unexpected};
use lockbook_core::{get_file_by_path, Error as CoreError, ExportDrawing, GetFileByPathError};
use std::fs;
use std::io::{stdout, Write};
use std::path::PathBuf;

pub fn export_drawing(drawing: &str, format: &str, destination: Option<PathBuf>) -> CliResult<()> {
    let file_metadata = get_file_by_path(&get_config(), drawing).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(drawing.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let (lockbook_format, extension) = get_image_format(format);

    match destination {
        None => {
            let destination_string = "/tmp/".to_string();

            lockbook_core::export_drawing(
                &get_config(),
                file_metadata.id,
                destination_string.clone(),
                lockbook_format,
            )
            .map_err(|export_drawing_err| match export_drawing_err {
                CoreError::UiError(err) => match err {
                    ExportDrawing::TreatedFolderAsDrawing => {
                        err!(DrawingTreatedAsFolder(drawing.to_string()))
                    }
                    ExportDrawing::NoAccount => err!(NoAccount),
                    ExportDrawing::FileDoesNotExist => err!(FileNotFound(drawing.to_string())),
                    ExportDrawing::DestinationIsDocument => err!(DestinationIsDocument),
                    ExportDrawing::DestinationDoesNotExist => err!(DestinationDoesNotExist),
                },
                CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
            })?;

            let drawing_true_name = match file_metadata.name.strip_suffix(".draw") {
                Some(name) => name,
                None => err_unexpected!("impossible").exit(),
            };

            let file_location =
                format!("{}{}.{}", destination_string, drawing_true_name, extension);

            let bytes = fs::read(file_location.as_str())
                .map_err(|err| err!(OsCouldNotReadFile(file_location.clone(), err)))?;

            stdout()
                .write_all(bytes.as_slice())
                .map_err(|err| err!(OsCouldNotWriteFile(file_location, err)))?;
        }
        Some(destination) => {
            let destination_string = match destination.to_str() {
                None => {
                    err_unexpected!("couldn't get destination as string: {:#?}", destination).exit()
                }
                Some(ok) => String::from(ok),
            };

            lockbook_core::export_drawing(
                &get_config(),
                file_metadata.id,
                destination_string,
                lockbook_format,
            )
            .map_err(|export_drawing_err| match export_drawing_err {
                CoreError::UiError(err) => match err {
                    ExportDrawing::TreatedFolderAsDrawing => {
                        err!(DrawingTreatedAsFolder(drawing.to_string()))
                    }
                    ExportDrawing::NoAccount => err!(NoAccount),
                    ExportDrawing::FileDoesNotExist => err!(FileNotFound(drawing.to_string())),
                    ExportDrawing::DestinationIsDocument => err!(DestinationIsDocument),
                    ExportDrawing::DestinationDoesNotExist => err!(DestinationDoesNotExist),
                },
                CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
            })?;
        }
    }

    Ok(())
}
