use crate::model::drawing::Drawing;
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::service::file_service::{DocumentUpdateError, FileService, ReadDocumentError};
use crate::storage::db_provider::Backend;

use image::codecs::farbfeld::FarbfeldEncoder;
use image::codecs::ico::IcoEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::pnm::PnmEncoder;
use image::codecs::tga::TgaEncoder;
use image::{ColorType, ImageError};
use raqote::{
    DrawOptions, DrawTarget, LineCap, LineJoin, PathBuilder, SolidSource, Source, StrokeStyle,
};
use std::io::BufWriter;
use uuid::Uuid;

pub enum SupportedImageFormats {
    Png,
    // Jpeg,
    Pnm,
    // Tiff,
    Tga,
    Ico,
    // Hdr,
    Farbfeld,
}

#[derive(Debug)]
pub enum DrawingError<MyBackend: Backend> {
    InvalidDrawingError(serde_json::error::Error),
    FailedToSaveDrawing(DocumentUpdateError<MyBackend>),
    FailedToRetrieveDrawing(ReadDocumentError<MyBackend>),
    FailedToEncodeImage(ImageError),
    CorruptedDrawing,
}

pub trait DrawingService<
    MyBackend: Backend,
    MyFileService: FileService<MyBackend>,
    FileMetadataDb: FileMetadataRepo<MyBackend>,
>
{
    fn save_drawing(
        backend: &MyBackend::Db,
        id: Uuid,
        serialized_drawing: &[u8],
    ) -> Result<(), DrawingError<MyBackend>>;
    fn get_drawing(backend: &MyBackend::Db, id: Uuid) -> Result<Drawing, DrawingError<MyBackend>>;
    fn export_drawing(
        backend: &MyBackend::Db,
        id: Uuid,
        format: SupportedImageFormats,
    ) -> Result<Vec<u8>, DrawingError<MyBackend>>;
}

pub struct DrawingServiceImpl<
    MyBackend: Backend,
    MyFileService: FileService<MyBackend>,
    FileMetadataDb: FileMetadataRepo<MyBackend>,
> {
    _backend: MyBackend,
    _file_service: MyFileService,
    _file_metadata_db: FileMetadataDb,
}

impl<
        MyBackend: Backend,
        MyFileService: FileService<MyBackend>,
        FileMetadataDb: FileMetadataRepo<MyBackend>,
    > DrawingService<MyBackend, MyFileService, FileMetadataDb>
    for DrawingServiceImpl<MyBackend, MyFileService, FileMetadataDb>
{
    fn save_drawing(
        backend: &MyBackend::Db,
        id: Uuid,
        drawing_bytes: &[u8],
    ) -> Result<(), DrawingError<MyBackend>> {
        let drawing_string = String::from(String::from_utf8_lossy(&drawing_bytes));

        serde_json::from_str::<Drawing>(drawing_string.as_str()) // validating json
            .map_err(DrawingError::InvalidDrawingError)?;

        MyFileService::write_document(backend, id, drawing_bytes)
            .map_err(DrawingError::FailedToSaveDrawing)
    }

    fn get_drawing(backend: &MyBackend::Db, id: Uuid) -> Result<Drawing, DrawingError<MyBackend>> {
        let drawing_bytes = MyFileService::read_document(backend, id)
            .map_err(DrawingError::FailedToRetrieveDrawing)?;

        let serialized_drawing = String::from(String::from_utf8_lossy(&drawing_bytes));

        serde_json::from_str::<Drawing>(serialized_drawing.as_str())
            .map_err(DrawingError::InvalidDrawingError)
    }

    fn export_drawing(
        backend: &MyBackend::Db,
        id: Uuid,
        format: SupportedImageFormats,
    ) -> Result<Vec<u8>, DrawingError<MyBackend>> {
        let drawing = Self::get_drawing(backend, id)?;

        let mut draw_target = DrawTarget::new(2125, 2750);

        let mut greatest_width = 5f32;
        let mut greatest_height = 5f32;

        for event in drawing.events {
            match event.stroke {
                Some(stroke) => {
                    let mut index = 3;
                    let (r, g, b, a) = Self::i32_byte_to_u8_byte(stroke.color);

                    while index < stroke.points.len() {
                        let mut pb = PathBuilder::new();
                        let x1 = stroke
                            .points
                            .get(index - 2)
                            .ok_or(DrawingError::CorruptedDrawing)?
                            .to_owned();
                        let y1 = stroke
                            .points
                            .get(index - 1)
                            .ok_or(DrawingError::CorruptedDrawing)?
                            .to_owned();
                        let x2 = stroke
                            .points
                            .get(index + 1)
                            .ok_or(DrawingError::CorruptedDrawing)?
                            .to_owned();
                        let y2 = stroke
                            .points
                            .get(index + 2)
                            .ok_or(DrawingError::CorruptedDrawing)?
                            .to_owned();

                        if x1 > greatest_width {
                            greatest_width = x1;
                        }

                        if x2 > greatest_width {
                            greatest_width = x2;
                        }

                        if y1 > greatest_height {
                            greatest_height = y1;
                        }

                        if y2 > greatest_height {
                            greatest_height = y2;
                        }

                        pb.move_to(x1, y1);
                        pb.line_to(x2, y2);

                        pb.close();
                        let path = pb.finish();

                        draw_target.stroke(
                            &path,
                            &Source::Solid(SolidSource { r, g, b, a }),
                            &StrokeStyle {
                                cap: LineCap::Round,
                                join: LineJoin::Round,
                                width: stroke
                                    .points
                                    .get(index + 2)
                                    .ok_or(DrawingError::CorruptedDrawing)?
                                    .to_owned(),
                                miter_limit: 10.0,
                                dash_array: Vec::new(),
                                dash_offset: 0.0,
                            },
                            &DrawOptions::new(),
                        );

                        index += 3;
                    }
                }
                None => continue,
            }
        }

        let mut drawing_bytes: Vec<u8> = Vec::new();

        for pixel in draw_target.into_vec().iter() {
            let (r, g, b, a) = Self::i32_byte_to_u8_byte(pixel.to_owned() as i32);

            drawing_bytes.push(r);
            drawing_bytes.push(g);
            drawing_bytes.push(b);
            drawing_bytes.push(a);
        }

        let mut buffer = Vec::<u8>::new();
        let buf_writer = BufWriter::new(&mut buffer);

        greatest_width += 20f32;
        greatest_height += 20f32;

        match format {
            SupportedImageFormats::Png => PngEncoder::new(buf_writer).encode(drawing_bytes.as_slice(), greatest_width as u32, greatest_height as u32, ColorType::Rgba8)
                ,
            SupportedImageFormats::Pnm => PnmEncoder::new(buf_writer).encode(drawing_bytes.as_slice(), greatest_width as u32, greatest_height as u32, ColorType::Rgba8)
                ,
            // SupportedImageFormats::Tiff => TiffEncoder::new(buf_writer).encode(drawing_bytes.as_slice(), greatest_width as u32, greatest_height as u32, ColorType::Rgba8)
            //     .map_err(DrawingError::FailedToEncodeImage),
            SupportedImageFormats::Tga => TgaEncoder::new(buf_writer).encode(drawing_bytes.as_slice(), greatest_width as u32, greatest_height as u32, ColorType::Rgba8)
                ,
            SupportedImageFormats::Ico => IcoEncoder::new(buf_writer).encode(drawing_bytes.as_slice(), greatest_width as u32, greatest_height as u32, ColorType::Rgba8)
                ,
            // SupportedImageFormats::Hdr => HdrEncoder::new(buf_writer).encode(drawing_bytes.as_slice(), greatest_width + 20, greatest_height + 20, ColorType::Rgba8)
            //     .map_err(DrawingError::FailedToEncodeImage),
            SupportedImageFormats::Farbfeld => FarbfeldEncoder::new(buf_writer).encode(drawing_bytes.as_slice(), greatest_width as u32, greatest_height as u32)
                ,
        }.map_err(DrawingError::FailedToEncodeImage)?;

        // SupportedImageFormats::Dds => ,
        // SupportedImageFormats::Bmp => ,
        // SupportedImageFormats::Avif => ,
        // SupportedImageFormats::Jpeg => image::codecs::,
        // SupportedImageFormats::WebP => ,

        Ok(buffer)
    }
}

impl<
        MyBackend: Backend,
        MyFileService: FileService<MyBackend>,
        FileMetadataDb: FileMetadataRepo<MyBackend>,
    > DrawingServiceImpl<MyBackend, MyFileService, FileMetadataDb>
{
    fn i32_byte_to_u8_byte(i32_byte: i32) -> (u8, u8, u8, u8) {
        let mut byte_1 = (i32_byte >> 16) & 0xffi32;
        let mut byte_2 = (i32_byte >> 8) & 0xffi32;
        let mut byte_3 = i32_byte & 0xffi32;
        let byte_4 = (i32_byte >> 24) & 0xffi32;

        if byte_4 > 0i32 {
            byte_1 = byte_1 * 255i32 / byte_4;
            byte_2 = byte_2 * 255i32 / byte_4;
            byte_3 = byte_3 * 255i32 / byte_4;
        }

        (byte_1 as u8, byte_2 as u8, byte_3 as u8, byte_4 as u8)
    }
}
