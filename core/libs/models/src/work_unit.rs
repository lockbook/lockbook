use serde::{Deserialize, Serialize};

use crate::file_metadata::DecryptedFileMetadata;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "tag", content = "content")]
pub enum WorkUnit {
    LocalChange { metadata: DecryptedFileMetadata },
    ServerChange { metadata: DecryptedFileMetadata },
}

impl WorkUnit {
    pub fn get_metadata(&self) -> DecryptedFileMetadata {
        match self {
            WorkUnit::LocalChange { metadata } => metadata,
            WorkUnit::ServerChange { metadata } => metadata,
        }
        .clone()
    }
}
