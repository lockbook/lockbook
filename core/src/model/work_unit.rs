use crate::model::file_metadata::FileMetadata;
use crate::model::local_changes::LocalChange;
use serde::Serialize;
use std::cmp::Ordering;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub enum WorkUnit {
    LocalChange { metadata: FileMetadata },

    ServerChange { metadata: FileMetadata },
}
