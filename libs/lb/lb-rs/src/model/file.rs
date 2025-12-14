use super::account::Username;
use super::file_metadata::FileType;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Copy)]
#[repr(C)]
pub enum ShareMode {
    Write,
    Read,
}

impl FromStr for ShareMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Write" => Ok(ShareMode::Write),
            "Read" => Ok(ShareMode::Read),
            _ => Err(()),
        }
    }
}

impl fmt::Display for ShareMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct Share {
    pub mode: ShareMode,
    pub shared_by: Username,
    pub shared_with: Username,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct File {
    pub id: Uuid,
    pub parent: Uuid,
    pub name: String,
    pub file_type: FileType,
    pub last_modified: u64,
    pub last_modified_by: Username,
    pub owner: Username,
    pub shares: Vec<Share>,
    pub size_bytes: u64,
}

impl File {
    pub fn is_document(&self) -> bool {
        self.file_type == FileType::Document
    }

    pub fn is_folder(&self) -> bool {
        self.file_type == FileType::Folder
    }

    pub fn is_root(&self) -> bool {
        self.id == self.parent
    }
}
