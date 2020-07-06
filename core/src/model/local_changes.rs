use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LocalChange {
    pub id: Uuid,
    pub renamed: Option<Renamed>,
    pub moved: Option<Moved>,
    pub new: bool,
    pub content_edited: bool,
    pub deleted: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Renamed {
    pub old_value: String,
}

impl From<String> for Renamed {
    fn from(s: String) -> Self {
        Renamed { old_value: s }
    }
}

impl From<&str> for Renamed {
    fn from(s: &str) -> Self {
        Renamed {
            old_value: String::from(s),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Moved {
    pub old_value: Uuid,
}

impl From<Uuid> for Moved {
    fn from(id: Uuid) -> Self {
        Moved { old_value: id }
    }
}
