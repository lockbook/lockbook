use crate::model::drawing::Drawing;
use crate::service::file_service::{DocumentUpdateError, FileService, ReadDocumentError};
use crate::storage::db_provider::Backend;
use raqote::{
    DrawOptions, DrawTarget, LineCap, LineJoin, PathBuilder, SolidSource, Source, StrokeStyle,
};
use uuid::Uuid;

pub type DrawingData = Vec<u32>;

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

                    while index < stroke.points.len() {
                        let mut pb = PathBuilder::new();
                        pb.move_to(stroke.points[index - 2], stroke.points[index - 1]);
                        pb.line_to(stroke.points[index + 1], stroke.points[index + 2]);

                        pb.close();
                        let path = pb.finish();

                        draw_target.stroke(
                            &path,
                            &Source::Solid(SolidSource {
                                r: 0x0,
                                g: 0x0,
                                b: 0x80,
                                a: 0x80,
                            }),
                            &StrokeStyle {
                                cap: LineCap::Round,
                                join: LineJoin::Round,
                                width: index as f32,
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

        Ok(draw_target.into_vec())
    }
}

// let mut pb = PathBuilder::new();
//                     pb.move_to(100., 100.);
//                     pb.line_to(300., 300.);
//                     pb.line_to(200., 300.);
//                     let path = pb.finish();
//
//                     dt.stroke(
//                         &path,
//                         &Source::Solid(SolidSource {
//                             r: 0x0,
//                             g: 0x0,
//                             b: 0x80,
//                             a: 0x80,
//                         }),
//                         &StrokeStyle {
//                             cap: LineCap::Round,
//                             join: LineJoin::Round,
//                             width: 10.,
//                             miter_limit: 2.,
//                             dash_array: vec![10., 18.],
//                             dash_offset: 16.,
//                         },
//                         &DrawOptions::new()
//                     );
