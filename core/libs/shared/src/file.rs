use crate::account::Username;
use crate::file_metadata::FileType;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct File {
    pub id: Uuid,
    pub parent: Uuid,
    pub name: String,
    pub file_type: FileType,
    pub last_modified: u64,
    pub last_modified_by: Username,
}
