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

impl TreeLike for SignedFile {
    type F = Self;

    fn ids(&self) -> HashSet<&Uuid> {
        let mut hashset = HashSet::new();
        hashset.insert(self.id());
        hashset
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&SignedFile> {
        if id == self.id() {
            Some(&self)
        } else {
            None
        }
    }

    fn insert(&mut self, f: SignedFile) -> Option<SignedFile> {
        *self = f;
        None
    }

    fn remove(&mut self, id: Uuid) -> Option<SignedFile> {
        unimplemented!()
    }
}

impl Stagable for SignedFile {}
