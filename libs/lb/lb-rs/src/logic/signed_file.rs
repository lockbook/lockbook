use crate::logic::crypto::ECSigned;
use crate::logic::file_like::FileLike;
use crate::logic::file_metadata::FileMetadata;
use crate::logic::tree_like::{TreeLike, TreeLikeMut};
use crate::logic::SharedResult;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

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

impl<F> TreeLike for Option<F>
where
    F: FileLike,
{
    type F = F;

    fn ids(&self) -> HashSet<&Uuid> {
        let mut hashset = HashSet::new();
        if let Some(f) = self {
            hashset.insert(f.id());
        }
        hashset
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&F> {
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
}

impl<F> TreeLikeMut for Option<F>
where
    F: FileLike,
{
    fn insert(&mut self, f: F) -> SharedResult<Option<F>> {
        Ok(self.replace(f))
    }

    fn remove(&mut self, id: Uuid) -> SharedResult<Option<F>> {
        if let Some(f) = self {
            if &id == f.id() {
                Ok(self.take())
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn clear(&mut self) -> SharedResult<()> {
        *self = None;
        Ok(())
    }
}
