use crate::model::drawing::Drawing;
use crate::service::file_service::{DocumentUpdateError, FileService, ReadDocumentError};
use crate::storage::db_provider::Backend;
use uuid::Uuid;

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
}
