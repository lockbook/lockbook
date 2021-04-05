use crate::file_metadata::FileMetadata;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "tag", content = "content")]
pub enum WorkUnit {
    LocalChange { metadata: FileMetadata },
    ServerChange { metadata: FileMetadata },
}

impl WorkUnit {
    pub fn get_metadata(&self) -> FileMetadata {
        match self {
            WorkUnit::LocalChange { metadata } => metadata,
            WorkUnit::ServerChange { metadata } => metadata,
        }
        .clone()
    }
}
