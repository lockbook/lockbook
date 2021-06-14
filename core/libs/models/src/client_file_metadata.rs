use crate::account::Username;
use crate::file_metadata::FileType;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ClientFileMetadata {
    pub id: Uuid,
    pub file_type: FileType,
    pub parent: Uuid,
    pub name: String,
    pub owner: String,
    pub metadata_version: u64,
    pub content_version: u64,
    pub deleted: bool,
    pub users_with_access: Vec<Username>,
}
