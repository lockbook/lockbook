use crate::account::Username;
use crate::crypto::{FolderAccessInfo, SignedValue, UserAccessInfo};
use serde::{Deserialize, Serialize};
use std::clone::Clone;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize, Copy)]
pub enum FileType {
    Document,
    Folder,
}

impl FromStr for FileType {
    type Err = ();
    fn from_str(input: &str) -> Result<FileType, Self::Err> {
        match input {
            "Document" => Ok(FileType::Document),
            "Folder" => Ok(FileType::Folder),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct FileMetadata {
    pub id: Uuid,
    pub file_type: FileType,
    pub parent: Uuid,
    pub name: String,
    pub owner: String,
    pub signature: SignedValue,
    pub metadata_version: u64,
    pub content_version: u64,
    pub deleted: bool,
    pub user_access_keys: HashMap<Username, UserAccessInfo>,
    pub folder_access_keys: FolderAccessInfo,
}
