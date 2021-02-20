use crate::error::CliResult;
use crate::utils::{get_config, get_image_format, SupportedImageFormats};
use crate::{err, err_unexpected};
use image::{ImageBuffer, ImageError, ImageFormat, Rgba};
use lockbook_core::{
    get_drawing_data, get_file_by_path, Error as CoreError, GetDrawingDataError, GetFileByPathError,
};
use std::fs;
use std::fs::File;
use std::io::{stdout, Write};
use std::path::PathBuf;
use uuid::Uuid;

pub fn export_drawing(drawing: &str, format: &str, destination: Option<PathBuf>) -> CliResult<()> {
    let file_metadata = get_file_by_path(&get_config(), drawing).map_err(|err| match err {
        CoreError::UiError(GetFileByPathError::NoFileAtThatPath) => {
            err!(FileNotFound(drawing.to_string()))
        }
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    match destination {
        None => {
            let directory_location = format!("/tmp/{}", Uuid::new_v4().to_string());
            fs::create_dir(&directory_location).map_err(|err| {
                err_unexpected!("couldn't open temporary file for writing: {:#?}", err)
            })?;

            let file_location = format!("{}/{}", directory_location, file_metadata.name);

            img.save_with_format(file_location.as_str(), image_format)
                .map_err(|err| err_unexpected!("{:?}", err))?;

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

            lockbook_core::export_drawing(&get_config(), file_metadata.id, destination_string, get_image_format(format)).map_err(|err| match err {

            });
        }
    }

    Ok(())
}
