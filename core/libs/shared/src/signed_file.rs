use crate::crypto::ECSigned;
use crate::file_like::FileLike;
use crate::file_metadata::FileMetadata;
use crate::tree_like::{Stagable, TreeLike};
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

pub type SignedFile = ECSigned<FileMetadata>;

impl Display for SignedFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

impl TreeLike for Option<SignedFile> {
    type F = SignedFile;

    fn ids(&self) -> HashSet<&Uuid> {
        let mut hashset = HashSet::new();
        if let Some(f) = self {
            hashset.insert(f.id());
        }
        hashset
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&SignedFile> {
        if let Some(f) = self {
            if id == f.id() {
                self.as_ref()
            } else {
                None
            }
        } else {
            None
        }
    }

    fn insert(&mut self, f: SignedFile) -> Option<SignedFile> {
        self.replace(f)
    }

    fn remove(&mut self, id: Uuid) -> Option<SignedFile> {
        if let Some(f) = self {
            if &id == f.id() {
                self.take()
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl Stagable for Option<SignedFile> {}
