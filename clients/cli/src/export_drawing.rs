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

    let drawing_data =
        get_drawing_data(&get_config(), file_metadata.id).map_err(|err| match err {
            CoreError::UiError(err) => match err {
                GetDrawingDataError::InvalidDrawing => err!(InvalidDrawing(drawing.to_string())),
                GetDrawingDataError::TreatedFolderAsDrawing => {
                    err!(DrawingTreatedAsFolder(drawing.to_string()))
                }
                GetDrawingDataError::NoAccount => err!(NoAccount),
                GetDrawingDataError::FileDoesNotExist => err!(FileNotFound(drawing.to_string())),
            },
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
        })?;

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        match ImageBuffer::from_vec(2125, 2750, drawing_data.clone()) {
            Some(image_buffer) => image_buffer,
            None => {
                err_unexpected!("Unable to use drawing data to construct an image buffer.").exit()
            }
        };

    let drawing_true_name = match file_metadata.name.strip_suffix(".draw") {
        Some(name) => name,
        None => err_unexpected!("impossible").exit(),
    };

    let (lockbook_format, extension) = get_image_format(format);

    let image_format = match lockbook_format {
        SupportedImageFormats::Png => ImageFormat::Png,
        SupportedImageFormats::Jpeg => ImageFormat::Jpeg,
        SupportedImageFormats::Bmp => ImageFormat::Bmp,
        SupportedImageFormats::Tga => ImageFormat::Tga,
    };

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

            if destination.is_file() {
                err!(DocTreatedAsFolder(destination_string)).exit()
            }

            let new_drawing_path =
                format!("{}/{}.{}", destination_string, drawing_true_name, extension);

            File::create(new_drawing_path.clone())
                .map_err(|err| err!(OsCouldNotCreateFile(new_drawing_path.clone(), err)))?;

            img.save_with_format(new_drawing_path, image_format)
                .map_err(|err| match err {
                    ImageError::Decoding(_)
                    | ImageError::Encoding(_)
                    | ImageError::Parameter(_)
                    | ImageError::Limits(_)
                    | ImageError::Unsupported(_)
                    | ImageError::IoError(_) => {
                        err_unexpected!("{:?}", err)
                    }
                })?
        }
    }

    CliResult::Ok(())
}
