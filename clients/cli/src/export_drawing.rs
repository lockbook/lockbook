use crate::error::CliResult;
use crate::utils::get_config;
use crate::{err, err_unexpected};
use image::buffer::Pixels;
use image::codecs::hdr::Rgbe8Pixel;
use image::{ImageBuffer, ImageError, ImageFormat, Rgba};
use lockbook_core::{
    get_drawing_data, get_file_by_path, Error as CoreError, GetDrawingDataError, GetFileByPathError,
};
use std::path::PathBuf;

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

    // let p = drawing_data[..].as_ptr();
    // let len = drawing_data[..].len();
    // // we want to return an [u8] slice instead of a [u32] slice. This is a safe thing to
    // // do because requirements of a [u32] slice are stricter.
    // let drawing_data_refined = unsafe { std::slice::from_raw_parts(p as *const u8, len * std::mem::size_of::<u32>()) };
    let drawing_data_refined = unsafe { drawing_data.align_to::<u8>().1 };

    let img = ImageBuffer::from_fn(2125, 2750, |x, y| {
        image::Rgba([
            drawing_data_refined[(x * (y + 1)) as usize],
            drawing_data_refined[((x + 1) * (y + 1)) as usize],
            drawing_data_refined[((x + 2) * (y + 1)) as usize],
            drawing_data_refined[((x + 3) * (y + 1)) as usize],
        ])
    });

    img.save_with_format(destination, ImageFormat::Png)
        .map_err(|err| match err {
            ImageError::Decoding(_)
            | ImageError::Encoding(_)
            | ImageError::Parameter(_)
            | ImageError::Limits(_)
            | ImageError::Unsupported(_)
            | ImageError::IoError(_) => err_unexpected!("{:?}", err),
        })
}
