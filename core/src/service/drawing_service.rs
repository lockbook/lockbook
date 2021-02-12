use crate::model::drawing::Drawing;

#[derive(Debug)]
pub enum DrawingError {
    InvalidDrawingError
}

pub trait DrawingService {
    fn validate_drawing(serialized_drawing: &str) -> Result<(), DrawingError>;
}

pub struct DrawingServiceImpl;

impl DrawingService for DrawingServiceImpl {
    fn validate_drawing(serialized_drawing: &str) -> Result<(), DrawingError> {
        match serde_json::<Drawing>::from_str(serialized_drawing) {
            Ok(_) => Ok(()),
            Err(_) => Err(DrawingError::InvalidDrawingError)
        }
    }
}
