use crate::model::crypto::ECSigned;
use crate::model::file_like::FileLike;
use crate::model::file_metadata::FileMetadata;
use crate::model::tree_like::{TreeLike, TreeLikeMut};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

use super::errors::LbResult;

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

    fn ids(&self) -> Vec<Uuid> {
        match self {
            Some(f) => vec![*f.id()],
            None => vec![],
        }
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&F> {
        if let Some(f) = self { if id == f.id() { self.as_ref() } else { None } } else { None }
    }
}

impl<F> TreeLikeMut for Option<F>
where
    F: FileLike,
{
    fn insert(&mut self, f: F) -> LbResult<Option<F>> {
        Ok(self.replace(f))
    }

    fn remove(&mut self, id: Uuid) -> LbResult<Option<F>> {
        if let Some(f) = self {
            if &id == f.id() { Ok(self.take()) } else { Ok(None) }
        } else {
            Ok(None)
        }
    }

    fn clear(&mut self) -> LbResult<()> {
        *self = None;
        Ok(())
    }
}
