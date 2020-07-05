use crate::model::file_metadata::FileMetadata;
use crate::model::local_changes::LocalChange;
use serde::Serialize;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub enum WorkUnit {
    LocalChange {
        metadata: FileMetadata,
        change_description: LocalChange,
    },

    ServerChange {
        metadata: FileMetadata,
    },
}
