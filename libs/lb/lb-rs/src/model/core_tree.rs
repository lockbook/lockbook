use std::hash::BuildHasher;

use crate::model::file_like::FileLike;
use crate::model::tree_like::{TreeLike, TreeLikeMut};
use db_rs::views::hashmap::DbHashMap;
use serde::Serialize;
use uuid::Uuid;

use super::errors::{LbResult, Unexpected};

impl<F, S> TreeLike for DbHashMap<Uuid, F, S>
where
    F: FileLike + Serialize,
    S: BuildHasher + Default,
{
    type F = F;

    fn ids(&self) -> Vec<Uuid> {
        self.iter().map(|(key, _)| key).copied().collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        self.get(id)
    }
}

impl<F, S> TreeLikeMut for DbHashMap<Uuid, F, S>
where
    F: FileLike + Serialize,
    S: BuildHasher + Default,
{
    fn insert(&mut self, f: Self::F) -> LbResult<Option<Self::F>> {
        DbHashMap::insert(self, *f.id(), f).map_unexpected()
    }

    fn remove(&mut self, id: Uuid) -> LbResult<Option<Self::F>> {
        DbHashMap::remove(self, &id).map_unexpected()
    }

    fn clear(&mut self) -> LbResult<()> {
        DbHashMap::clear(self).map_unexpected()
    }
}
