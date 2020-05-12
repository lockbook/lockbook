use serde::{Deserialize, Serialize};
use std::clone::Clone;
use uuid::Uuid;

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ClientFileMetadata {
    /// Immutable unique identifier for everything related to this file, TODO UUID
    pub file_id: String,

    /// Human readable name for this file. Does not need to be unique TODO make this encrypted / hashed / etc.
    pub file_name: String,

    /// Where this file lives relative to your other files. TODO make this encrypted / hashed / etc.
    pub file_path: String,

    /// DB generated timestamp representing the last time the content of a file was updated
    pub file_content_version: u64,

    /// DB generated timestamp representing the last time the metadata for this file changed
    pub file_metadata_version: u64,

    /// True if this is a new file, that has never been synced before
    pub new_file: bool,

    /// True if there are changes to content that need to be synced
    pub content_edited_locally: bool,

    /// True if there are changes to metadata that need to be synced
    pub metadata_edited_locally: bool,

    /// True if the user attempted to delete this file locally. Once the server also deletes this file, the content and the associated metadata are deleted locally.
    pub deleted_locally: bool,
}

impl ClientFileMetadata {
    pub fn new_file(name: &String, path: &String) -> ClientFileMetadata {
        let version = 0;
        let id = Uuid::new_v4().to_string();
        ClientFileMetadata {
            file_id: id.to_string(),
            file_name: name.clone(),
            file_path: path.clone(),
            file_content_version: version.clone(),
            file_metadata_version: version.clone(),
            new_file: true,
            content_edited_locally: false,
            metadata_edited_locally: false,
            deleted_locally: false,
        }
    }
}
