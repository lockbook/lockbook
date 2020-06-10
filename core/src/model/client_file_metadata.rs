use serde::{Deserialize, Serialize};
use std::clone::Clone;
use uuid::Uuid;

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ClientFileMetadata {
    /// Immutable unique identifier for everything related to this file, TODO UUID
    pub id: String,

    /// Human readable name for this file. Does not need to be unique TODO make this encrypted / hashed / etc.
    pub name: String,

    /// Where this file lives relative to your other files. TODO make this encrypted / hashed / etc.
    pub parent_id: String,

    /// DB generated timestamp representing the last time the content of a file was updated
    pub content_version: u64,

    /// DB generated timestamp representing the last time the metadata for this file changed
    pub metadata_version: u64,

    /// True if this is a new file, that has never been synced before
    pub new: bool,

    /// True if there are changes to content that need to be synced
    pub document_edited: bool,

    /// True if there are changes to metadata that need to be synced
    pub metadata_changed: bool,

    /// True if the user attempted to delete this file locally. Once the server also deletes this file, the content and the associated metadata are deleted locally.
    pub deleted: bool,
}

impl ClientFileMetadata {
    pub fn new_file(name: &String, path: &String) -> ClientFileMetadata {
        let version = 0;
        let id = Uuid::new_v4().to_string();
        ClientFileMetadata {
            id: id,
            name: name.clone(),
            parent_id: path.clone(),
            content_version: version,
            metadata_version: version,
            new: true,
            document_edited: false,
            metadata_changed: false,
            deleted: false,
        }
    }
}
