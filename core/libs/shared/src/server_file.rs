use crate::crypto::ECSigned;
use crate::file_like::FileLike;
use crate::file_metadata::FileMetadata;
use crate::signed_file::SignedFile;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ServerFile {
    pub file: ECSigned<FileMetadata>,
    pub metadata_version: u64,
    pub content_version: u64,
}

pub trait IntoServerFile {
    fn add_time(self, version: u64) -> ServerFile;
}

impl IntoServerFile for SignedFile {
    fn add_time(self, version: u64) -> ServerFile {
        ServerFile { file: self, metadata_version: version, content_version: version }
    }
}

trait FromSignedFile {
    fn from(file: SignedFile, version: u64) -> ServerFile {
        file.add_time(version)
    }
}

impl Display for ServerFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}
