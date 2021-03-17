use crate::model::drawing::{ColorAlias, ColorRGB, Drawing, Stroke};
use crate::repo::file_metadata_repo::FileMetadataRepo;
use crate::service::file_service::{DocumentUpdateError, FileService, ReadDocumentError};
use crate::storage::db_provider::Backend;

use image::codecs::bmp::BmpEncoder;
use image::codecs::farbfeld::FarbfeldEncoder;

use image::codecs::jpeg::JpegEncoder;
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
    Jpeg,
    Pnm,
    Tga,
    Farbfeld,
    Bmp,
}

#[derive(Debug)]
pub enum DrawingError<MyBackend: Backend> {
    InvalidDrawingError(serde_json::error::Error),
    FailedToSaveDrawing(DocumentUpdateError<MyBackend>),
    FailedToRetrieveDrawing(ReadDocumentError<MyBackend>),
    FailedToEncodeImage(ImageError),
    UnequalPointsAndGirthMetrics,
    UnableToGetColorFromAlias,
    CorruptedDrawing,
}

macro_rules! hashmap {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = ::std::collections::HashMap::new();
         $( map.insert($key, $val); )*
         map
    }}
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

        let drawing_string = String::from(String::from_utf8_lossy(&drawing_bytes));

        serde_json::from_str::<Drawing>(drawing_string.as_str())
            .map_err(DrawingError::InvalidDrawingError)
    }

    fn export_drawing(
        backend: &MyBackend::Db,
        id: Uuid,
        format: SupportedImageFormats,
    ) -> Result<Vec<u8>, DrawingError<MyBackend>> {
        let drawing = Self::get_drawing(backend, id)?;

        let theme = drawing.theme.unwrap_or_else(|| {
            hashmap![
                ColorAlias::White => ColorRGB{r: 0xFF, g: 0xFF, b: 0xFF},
                ColorAlias::Black => ColorRGB{r: 0x00, g: 0x00, b: 0x00},
                ColorAlias::Red => ColorRGB{r: 0xFF, g: 0x00, b: 0x00},
                ColorAlias::Green => ColorRGB{r: 0x00, g: 0xFF, b: 0x00},
                ColorAlias::Yellow => ColorRGB{r: 0xFF, g: 0xFF, b: 0x00},
                ColorAlias::Blue => ColorRGB{r: 0x00, g: 0x00, b: 0xFF},
                ColorAlias::Magenta => ColorRGB{r: 0xFF, g: 0x00, b: 0xFF},
                ColorAlias::Cyan => ColorRGB{r: 0x00, g: 0xFF, b: 0xFF}
            ]
        });

        let (width, height) = Self::get_drawing_bounds(drawing.strokes.as_slice())?;

        let mut draw_target = DrawTarget::new(width as i32, height as i32);

        for stroke in drawing.strokes {
            let color_rgb = theme
                .get(&stroke.color)
                .ok_or(DrawingError::UnableToGetColorFromAlias)?;

            if stroke.points_x.len() != stroke.points_y.len()
                || stroke.points_y.len() != stroke.points_girth.len()
            {
                return Err(DrawingError::UnequalPointsAndGirthMetrics);
            }

            if stroke.alpha > 1.0 || stroke.alpha < 0.0 {
                return Err(DrawingError::CorruptedDrawing);
            }

            for point_index in 0..stroke.points_x.len() - 1 {
                let mut pb = PathBuilder::new();
                let x1 = stroke
                    .points_x
                    .get(point_index)
                    .ok_or(DrawingError::CorruptedDrawing)?
                    .to_owned();
                let y1 = stroke
                    .points_y
                    .get(point_index)
                    .ok_or(DrawingError::CorruptedDrawing)?
                    .to_owned();
                let x2 = stroke
                    .points_x
                    .get(point_index + 1)
                    .ok_or(DrawingError::CorruptedDrawing)?
                    .to_owned();
                let y2 = stroke
                    .points_y
                    .get(point_index + 1)
                    .ok_or(DrawingError::CorruptedDrawing)?
                    .to_owned();

                pb.move_to(x1, y1);
                pb.line_to(x2, y2);

                pb.close();
                let path = pb.finish();

                draw_target.stroke(
                    &path,
                    &Source::Solid(SolidSource {
                        r: color_rgb.r,
                        g: color_rgb.g,
                        b: color_rgb.b,
                        a: (stroke.alpha * 255.0) as u8,
                    }),
                    &StrokeStyle {
                        cap: LineCap::Round,
                        join: LineJoin::Round,
                        width: stroke
                            .points_girth
                            .get(point_index)
                            .ok_or(DrawingError::CorruptedDrawing)?
                            .to_owned(),
                        miter_limit: 10.0,
                        dash_array: Vec::new(),
                        dash_offset: 0.0,
                    },
                    &DrawOptions::new(),
                );
            }
        }

        let mut buffer = Vec::<u8>::new();
        let mut buf_writer = BufWriter::new(&mut buffer);

        let mut drawing_bytes: Vec<u8> = Vec::new();

        for pixel in draw_target.into_vec().iter() {
            let (r, g, b, a) = Self::u32_byte_to_u8_bytes(pixel.to_owned());

            drawing_bytes.push(r);
            drawing_bytes.push(g);
            drawing_bytes.push(b);
            drawing_bytes.push(a);
        }

        match format {
            SupportedImageFormats::Png => PngEncoder::new(&mut buf_writer).encode(
                drawing_bytes.as_slice(),
                width,
                height,
                ColorType::Rgba8,
            ),
            SupportedImageFormats::Pnm => PnmEncoder::new(&mut buf_writer).encode(
                drawing_bytes.as_slice(),
                width,
                height,
                ColorType::Rgba8,
            ),
            SupportedImageFormats::Jpeg => JpegEncoder::new(&mut buf_writer).encode(
                drawing_bytes.as_slice(),
                width,
                height,
                ColorType::Rgba8,
            ),
            SupportedImageFormats::Tga => TgaEncoder::new(&mut buf_writer).encode(
                drawing_bytes.as_slice(),
                width,
                height,
                ColorType::Rgba8,
            ),
            SupportedImageFormats::Farbfeld => FarbfeldEncoder::new(&mut buf_writer).encode(
                drawing_bytes.as_slice(),
                width,
                height,
            ),
            SupportedImageFormats::Bmp => BmpEncoder::new(&mut buf_writer).encode(
                drawing_bytes.as_slice(),
                width,
                height,
                ColorType::Rgba8,
            ),
        }
        .map_err(DrawingError::FailedToEncodeImage)?;

        std::mem::drop(buf_writer);

        Ok(buffer)
    }
}

impl<
        MyBackend: Backend,
        MyFileService: FileService<MyBackend>,
        FileMetadataDb: FileMetadataRepo<MyBackend>,
    > DrawingServiceImpl<MyBackend, MyFileService, FileMetadataDb>
{
    fn u32_byte_to_u8_bytes(u32_byte: u32) -> (u8, u8, u8, u8) {
        let mut byte_1 = (u32_byte >> 16) & 0xffu32;
        let mut byte_2 = (u32_byte >> 8) & 0xffu32;
        let mut byte_3 = u32_byte & 0xffu32;
        let byte_4 = (u32_byte >> 24) & 0xffu32;

        if byte_4 > 0u32 {
            byte_1 = byte_1 * 255u32 / byte_4;
            byte_2 = byte_2 * 255u32 / byte_4;
            byte_3 = byte_3 * 255u32 / byte_4;
        }

        (byte_1 as u8, byte_2 as u8, byte_3 as u8, byte_4 as u8)
    }

    fn get_drawing_bounds(strokes: &[Stroke]) -> Result<(u32, u32), DrawingError<MyBackend>> {
        let stroke_to_max_x = |stroke: &Stroke| {
            stroke
                .points_x
                .iter()
                .zip(stroke.points_girth.clone())
                .map(|(x, girth)| x + girth)
                .map(|num| num as u32)
                .max()
                .unwrap_or(0)
        };

        let stroke_to_max_y = |stroke: &Stroke| {
            stroke
                .points_y
                .iter()
                .zip(stroke.points_girth.clone())
                .map(|(y, girth)| y + girth)
                .map(|num| num as u32)
                .max()
                .unwrap_or(0)
        };

        let max_x_and_girth = strokes
            .iter()
            .map(|stroke| stroke_to_max_x(stroke))
            .max()
            .unwrap_or(0);

        let max_y_and_girth = strokes
            .iter()
            .map(|stroke| stroke_to_max_y(stroke))
            .max()
            .unwrap_or(0);

        Ok((max_x_and_girth + 20, max_y_and_girth + 20))
    }
}
