use crate::model::file_metadata::FileMetadata;

use serde::Serialize;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub enum WorkUnit {
    LocalChange { metadata: FileMetadata },

    ServerChange { metadata: FileMetadata },
}

impl WorkUnit {
    pub fn get_metadata(&self) -> FileMetadata {
        match self {
            WorkUnit::LocalChange { metadata } => metadata,
            WorkUnit::ServerChange { metadata } => metadata,
        }.clone()
    }
}
