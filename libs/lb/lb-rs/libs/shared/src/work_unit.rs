use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "tag", content = "content")]
pub enum WorkUnit {
    LocalChange(Uuid),
    ServerChange(Uuid),
}

impl WorkUnit {
    pub fn id(&self) -> Uuid {
        *match self {
            WorkUnit::LocalChange(id) => id,
            WorkUnit::ServerChange(id) => id,
        }
    }
}
