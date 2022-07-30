use crate::account::Username;
use crate::file_metadata::FileType;
use uuid::Uuid;

pub struct File {
    pub id: Uuid,
    pub parent: Uuid,
    pub name: String,
    pub file_type: FileType,
    pub last_modified: u64,
    pub last_modified_by: Username,
}
