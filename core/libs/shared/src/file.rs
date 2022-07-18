use crate::file_metadata::FileType;
use uuid::Uuid;

pub struct File {
    pub id: Uuid,
    pub parent: Uuid,
    pub name: String,
    pub file_type: FileType,
}
