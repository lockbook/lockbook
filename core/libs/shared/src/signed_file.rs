use crate::crypto::ECSigned;
use crate::file_like::FileLike;
use crate::file_metadata::FileMetadata;
use std::fmt::{Display, Formatter};

pub type SignedFile = ECSigned<FileMetadata>;

impl Display for SignedFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}
