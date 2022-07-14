use serde::{Deserialize, Serialize};

use crate::file_metadata::CoreFile;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "tag", content = "content")]
pub enum WorkUnit {
    LocalChange { metadata: CoreFile },
    ServerChange { metadata: CoreFile },
}

impl WorkUnit {
    pub fn get_metadata(&self) -> CoreFile {
        match self {
            WorkUnit::LocalChange { metadata } => metadata,
            WorkUnit::ServerChange { metadata } => metadata,
        }
        .clone()
    }
}

#[derive(Debug, Serialize, Clone)]
pub enum ClientWorkUnit {
    PullMetadata,
    PushMetadata,
    PullDocument(String),
    PushDocument(String),
}
