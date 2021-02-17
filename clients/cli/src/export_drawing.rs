use crate::error::CliResult;
use crate::utils::{get_config, get_image_format, SupportedImageFormats};
use crate::{err, err_unexpected, path_string};
use image::{ImageBuffer, ImageError, ImageFormat, Rgba};
use lockbook_core::{
    get_drawing_data, get_file_by_path, Error as CoreError, GetDrawingDataError, GetFileByPathError,
};
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::fs;

pub fn export_drawing(drawing: &str, destination: PathBuf, format: &str) -> CliResult<()> {
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
        ImageBuffer::from_vec(2125, 2750, drawing_data).unwrap();

    let drawing_true_name = match file_metadata.name.strip_suffix(".draw") {
        Some(name) => name,
        None => err_unexpected!("impossible").exit()
    };

    let (lockbook_format, friendly_format) = get_image_format(format);

    let image_format = match lockbook_format {
        SupportedImageFormats::Png => ImageFormat::Png,
        SupportedImageFormats::Jpeg => ImageFormat::Jpeg,
        SupportedImageFormats::Bmp => ImageFormat::Bmp,
        SupportedImageFormats::Tga => ImageFormat::Tga
    };

    let new_drawing_path = format!("{}/{}", destination.to_str().unwrap(), format!("{}.{}", drawing_true_name, friendly_format));

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
            },
        })
}
