use crate::model::file_metadata::FileMetadata;

use serde::Serialize;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub enum WorkUnit {
    LocalChange { metadata: FileMetadata },

    ServerChange { metadata: FileMetadata },
}