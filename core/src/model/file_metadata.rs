use serde::{Deserialize, Serialize};
use std::clone::Clone;

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct FileMetadata {
    pub id: String,
    pub name: String,
    pub path: String,
    pub updated_at: u64,
    pub version: u64,
    pub status: Status,
}

#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub enum Status {
    New,
    Local,
    Remote,
    Synced,
}
