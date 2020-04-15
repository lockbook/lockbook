use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Deserialize, Serialize)]
pub struct FileMetadata {
    pub id: String,
    pub name: String,
    pub path: String,
    pub updated_at: i64,
    pub status: String,
}
