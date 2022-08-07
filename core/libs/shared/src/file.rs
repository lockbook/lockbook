use crate::account::Username;
use crate::file_metadata::FileType;
use serde::{Deserialize, Serialize};
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

impl File {
    pub fn is_document(&self) -> bool {
        self.file_type == FileType::Document
    }

    pub fn is_folder(&self) -> bool {
        self.file_type == FileType::Folder
    }
}
