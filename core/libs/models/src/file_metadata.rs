use std::clone::Clone;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::account::Username;
use crate::crypto::{AESKey, EncryptedFolderAccessKey, SecretFileName, UserAccessInfo};

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

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct FileMetadata {
    pub id: Uuid,
    pub file_type: FileType,
    pub parent: Uuid,
    pub name: SecretFileName,
    pub owner: String,
    pub metadata_version: u64,
    pub content_version: u64,
    pub deleted: bool,
    pub user_access_keys: HashMap<Username, UserAccessInfo>,
    pub folder_access_keys: EncryptedFolderAccessKey,
}

impl fmt::Debug for FileMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileMetadata")
            .field("id", &self.id)
            .field("file_type", &self.file_type)
            .field("parent", &self.parent)
            .field("metadata_version", &self.metadata_version)
            .field("content_version", &self.content_version)
            .field("deleted", &self.deleted)
            .finish()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct DecryptedFileMetadata {
    pub id: Uuid,
    pub file_type: FileType,
    pub parent: Uuid,
    pub decrypted_name: String,
    pub owner: String,
    pub metadata_version: u64,
    pub content_version: u64,
    pub deleted: bool,
    pub decrypted_access_key: AESKey, // access key is the same whether it's decrypted for user or for folder
}

impl fmt::Debug for DecryptedFileMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecryptedFileMetadata")
            .field("id", &self.id)
            .field("file_type", &self.file_type)
            .field("parent", &self.parent)
            .field("decrypted_name", &self.decrypted_name)
            .field("metadata_version", &self.metadata_version)
            .field("content_version", &self.content_version)
            .field("deleted", &self.deleted)
            .finish()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct FileMetadataDiff {
    pub id: Uuid,
    pub file_type: FileType,
    pub old_parent_and_name: Option<(Uuid, SecretFileName)>,
    pub new_parent: Uuid,
    pub new_name: SecretFileName,
    pub new_deleted: bool,
    pub new_folder_access_keys: EncryptedFolderAccessKey,
}

impl FileMetadataDiff {
    pub fn new(metadata: &FileMetadata) -> Self {
        FileMetadataDiff {
            id: metadata.id,
            file_type: metadata.file_type,
            old_parent_and_name: None,
            new_parent: metadata.parent,
            new_name: metadata.name.clone(),
            new_deleted: metadata.deleted,
            new_folder_access_keys: metadata.folder_access_keys.clone(),
        }
    }

    pub fn new_diff(
        old_parent: Uuid,
        old_name: &SecretFileName,
        new_metadata: &FileMetadata,
    ) -> Self {
        FileMetadataDiff {
            id: new_metadata.id,
            file_type: new_metadata.file_type,
            old_parent_and_name: Some((old_parent, old_name.clone())),
            new_parent: new_metadata.parent,
            new_name: new_metadata.name.clone(),
            new_deleted: new_metadata.deleted,
            new_folder_access_keys: new_metadata.folder_access_keys.clone(),
        }
    }
}

impl fmt::Debug for FileMetadataDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileMetadataDiff")
            .field("id", &self.id)
            .field("file_type", &self.file_type)
            .field("new_parent", &self.new_parent)
            .field("new_deleted", &self.new_deleted)
            .field(
                "old_parent",
                &self.old_parent_and_name.clone().map(|(p, _)| p),
            )
            .field(
                "old_name",
                &self
                    .old_parent_and_name
                    .clone()
                    .map(|(_, n)| base64::encode(n.hmac)),
            )
            .finish()
    }
}
