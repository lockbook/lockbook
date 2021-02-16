use crate::error::CliResult;
use crate::utils::get_config;
use crate::{err, err_unexpected};
use image::{ImageBuffer, ImageError, ImageFormat, Rgba};
use lockbook_core::{
    get_drawing_data, get_file_by_path, Error as CoreError, GetDrawingDataError, GetFileByPathError,
};
use std::path::PathBuf;
use std::fs::OpenOptions;

pub enum SupportedImageFormats {
    Png,
    Jpeg,
    Pnm,
    Bmp,
    Farbfeld,
    Tga,
}

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


    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_vec(2125, 2750, drawing_data).unwrap();

    let new_drawing_path = format!("{}/{}", destination.to_str().unwrap(), file_metadata.name);

    println!("{}", new_drawing_path);

    if let Err(err) = OpenOptions::new().write(true).create_new(true).open(new_drawing_path.clone()) {
        err_unexpected!("WHAT").exit()
    }

    img.save_with_format(new_drawing_path, ImageFormat::Png)
        .map_err(|err| match err {
            ImageError::Decoding(_)
            | ImageError::Encoding(_)
            | ImageError::Parameter(_)
            | ImageError::Limits(_)
            | ImageError::Unsupported(_)
            | ImageError::IoError(_) => err_unexpected!("{:?}", err),
        })
}
