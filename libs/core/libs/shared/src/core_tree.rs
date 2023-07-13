use crate::file_like::FileLike;
use crate::tree_like::{TreeLike, TreeLikeMut};
use crate::SharedResult;
use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

impl<F> TreeLike for db_rs::LookupTable<Uuid, F>
where
    F: FileLike + Serialize,
{
    type F = F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.get().keys().collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        self.get().get(id)
    }
}

impl<F> TreeLikeMut for db_rs::LookupTable<Uuid, F>
where
    F: FileLike + Serialize,
{
    fn insert(&mut self, f: Self::F) -> SharedResult<Option<Self::F>> {
        Ok(db_rs::LookupTable::insert(self, *f.id(), f)?)
    }

    fn remove(&mut self, id: Uuid) -> SharedResult<Option<Self::F>> {
        Ok(db_rs::LookupTable::remove(self, &id)?)
    }

    fn clear(&mut self) -> SharedResult<()> {
        Ok(db_rs::LookupTable::clear(self)?)
    }
}
