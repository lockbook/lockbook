use std::hash::BuildHasher;

use crate::model::file_like::FileLike;
use crate::model::tree_like::{TreeLike, TreeLikeMut};
use serde::Serialize;
use uuid::Uuid;

use super::errors::{LbResult, Unexpected};

impl<F, S> TreeLike for db_rs::LookupTable<Uuid, F, S>
where
    F: FileLike + Serialize,
    S: BuildHasher + Default,
{
    type F = F;

    fn ids(&self) -> Vec<Uuid> {
        self.get().keys().copied().collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        self.get().get(id)
    }
}

impl<F, S> TreeLikeMut for db_rs::LookupTable<Uuid, F, S>
where
    F: FileLike + Serialize,
    S: BuildHasher + Default,
{
    fn insert(&mut self, f: Self::F) -> LbResult<Option<Self::F>> {
        db_rs::LookupTable::insert(self, *f.id(), f).map_unexpected()
    }

    fn remove(&mut self, id: Uuid) -> LbResult<Option<Self::F>> {
        db_rs::LookupTable::remove(self, &id).map_unexpected()
    }

    fn clear(&mut self) -> LbResult<()> {
        db_rs::LookupTable::clear(self).map_unexpected()
    }
}
