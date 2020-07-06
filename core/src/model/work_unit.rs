use crate::model::file_metadata::FileMetadata;

use serde::ser::SerializeStructVariant;
use serde::{Serialize, Serializer};

#[derive(Debug, Clone, PartialEq)]
pub enum WorkUnit {
    LocalChange { metadata: FileMetadata },

    ServerChange { metadata: FileMetadata },
}

impl Serialize for WorkUnit {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match *self {
            WorkUnit::LocalChange { ref metadata } => {
                let mut sv =
                    serializer.serialize_struct_variant("work_unit", 0, "local_change", 1)?;
                sv.serialize_field("metadata", metadata)?;
                sv.end()
            }
            WorkUnit::ServerChange { ref metadata } => {
                let mut sv =
                    serializer.serialize_struct_variant("work_unit", 1, "server_change", 1)?;
                sv.serialize_field("metadata", metadata)?;
                sv.end()
            }
        }
    }
}
