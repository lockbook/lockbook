use crate::model::drawing::Drawing;
use crate::service::file_service::{DocumentUpdateError, FileService, ReadDocumentError};
use crate::storage::db_provider::Backend;
use raqote::{
    DrawOptions, DrawTarget, LineCap, LineJoin, PathBuilder, SolidSource, Source, StrokeStyle,
};
use uuid::Uuid;

pub type DrawingData = Vec<u8>;

#[derive(Debug)]
pub enum DrawingError<MyBackend: Backend> {
    InvalidDrawingError(serde_json::error::Error),
    FailedToSaveDrawing(DocumentUpdateError<MyBackend>),
    FailedToRetrieveDrawing(ReadDocumentError<MyBackend>),
}

pub trait DrawingService<MyBackend: Backend, MyFileService: FileService<MyBackend>> {
    fn save_drawing(
        backend: &MyBackend::Db,
        id: Uuid,
        serialized_drawing: &str,
    ) -> Result<(), DrawingError<MyBackend>>;
    fn get_drawing(backend: &MyBackend::Db, id: Uuid) -> Result<Drawing, DrawingError<MyBackend>>;
    fn get_drawing_data(
        backend: &MyBackend::Db,
        id: Uuid,
    ) -> Result<DrawingData, DrawingError<MyBackend>>;
}

pub struct DrawingServiceImpl<MyBackend: Backend, MyFileService: FileService<MyBackend>> {
    _backend: MyBackend,
    _file_service: MyFileService,
}

impl<MyBackend: Backend, MyFileService: FileService<MyBackend>>
    DrawingService<MyBackend, MyFileService> for DrawingServiceImpl<MyBackend, MyFileService>
{
    fn save_drawing(
        backend: &MyBackend::Db,
        id: Uuid,
        serialized_drawing: &str,
    ) -> Result<(), DrawingError<MyBackend>> {
        serde_json::from_str::<Drawing>(serialized_drawing)
            .map_err(DrawingError::InvalidDrawingError)?;

        MyFileService::write_document(backend, id, serialized_drawing.as_bytes())
            .map_err(DrawingError::FailedToSaveDrawing)
    }

    fn get_drawing(backend: &MyBackend::Db, id: Uuid) -> Result<Drawing, DrawingError<MyBackend>> {
        let drawing_bytes = MyFileService::read_document(backend, id)
            .map_err(DrawingError::FailedToRetrieveDrawing)?;

        let serialized_drawing = String::from(String::from_utf8_lossy(&drawing_bytes));

        serde_json::from_str::<Drawing>(serialized_drawing.as_str())
            .map_err(DrawingError::InvalidDrawingError)
    }

    fn get_drawing_data(
        backend: &MyBackend::Db,
        id: Uuid,
    ) -> Result<DrawingData, DrawingError<MyBackend>> {
        let drawing = Self::get_drawing(backend, id)?;

        let mut draw_target = DrawTarget::new(2125, 2750);

        for event in drawing.events {
            match event.stroke {
                Some(stroke) => {
                    let mut index = 3;

                    let pixel_color: i32 = stroke.color;

                    let a_u32 = (pixel_color >> 24) & 0xffi32;
                    let mut r_u32 = (pixel_color >> 16) & 0xffi32;
                    let mut g_u32 = (pixel_color >> 8) & 0xffi32;
                    let mut b_u32 = (pixel_color >> 0) & 0xffi32;

                    if a_u32 > 0i32 {
                        r_u32 = r_u32 * 255i32 / a_u32;
                        g_u32 = g_u32 * 255i32 / a_u32;
                        b_u32 = b_u32 * 255i32 / a_u32;
                    }

                    let r = r_u32 as u8;
                    let g = g_u32 as u8;
                    let b = b_u32 as u8;
                    let a = a_u32 as u8;

                    while index < stroke.points.len() {
                        let mut pb = PathBuilder::new();
                        pb.move_to(stroke.points[index - 2], stroke.points[index - 1]);
                        pb.line_to(stroke.points[index + 1], stroke.points[index + 2]);

                        pb.close();
                        let path = pb.finish();

                        draw_target.stroke(
                            &path,
                            &Source::Solid(SolidSource {
                                r: r,
                                g: g,
                                b: b,
                                a: a,
                            }),
                            &StrokeStyle {
                                cap: LineCap::Round,
                                join: LineJoin::Round,
                                width: stroke.points[index] as f32,
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

        draw_target.into_vec().iter().for_each(|pixel| {
            let a = (pixel >> 24) & 0xffu32;
            let mut r = (pixel >> 16) & 0xffu32;
            let mut g = (pixel >> 8) & 0xffu32;
            let mut b = (pixel >> 0) & 0xffu32;

            if a > 0u32 {
                r = r * 255u32 / a;
                g = g * 255u32 / a;
                b = b * 255u32 / a;
            }

            drawing_bytes.push(r as u8);
            drawing_bytes.push(g as u8);
            drawing_bytes.push(b as u8);
            drawing_bytes.push(a as u8);
        });

        Ok(drawing_bytes)
    }
}
