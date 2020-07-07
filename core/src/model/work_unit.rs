use crate::model::file_metadata::FileMetadata;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "tag", content = "content")]
pub enum WorkUnit {
    LocalChange { metadata: FileMetadata },

    ServerChange { metadata: FileMetadata },
}
