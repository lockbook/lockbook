use crate::crypto::ECSigned;
use crate::file::like::FileLike;
use crate::file::metadata::FileMetadata;
use std::fmt::{Display, Formatter};

pub type SignedFile = ECSigned<FileMetadata>;

// Impl'd to avoid comparing encrypted
impl PartialEq for SignedFile {
    fn eq(&self, other: &Self) -> bool {
        self.timestamped_value.value == other.timestamped_value.value
            && self.public_key == other.public_key
    }
}

impl Display for SignedFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}
