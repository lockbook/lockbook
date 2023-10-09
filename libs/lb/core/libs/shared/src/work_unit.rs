use serde::{Deserialize, Serialize};

use crate::file::File;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "tag", content = "content")]
pub enum WorkUnit {
    LocalChange { metadata: File },
    ServerChange { metadata: File },
}

impl WorkUnit {
    pub fn get_metadata(&self) -> File {
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
    PullDocument(File),
    PushDocument(File),
}
