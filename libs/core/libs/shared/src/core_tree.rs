use crate::file_like::FileLike;
use crate::tree_like::{TreeLike, TreeLikeMut};
use hmdb::log::SchemaEvent;
use hmdb::transaction::TransactionTable;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

impl<F> TreeLike for db_rs::LookupTable<Uuid, F>
where
    F: FileLike + Serialize + DeserializeOwned,
{
    type F = F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.data().keys().collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        self.data().get(id)
    }
}

impl<F> TreeLikeMut for db_rs::LookupTable<Uuid, F>
where
    F: FileLike + Serialize + DeserializeOwned,
{
    fn insert(&mut self, f: Self::F) -> Option<Self::F> {
        db_rs::LookupTable::insert(self, *f.id(), f).unwrap() // todo: modify treelikemut
    }

    fn remove(&mut self, id: Uuid) -> Option<Self::F> {
        db_rs::LookupTable::remove(self, &id).unwrap()
    }

    fn clear(&mut self) {
        db_rs::LookupTable::clear(self).unwrap()
    }
}

impl<F, Log> TreeLike for TransactionTable<'_, Uuid, F, Log>
where
    F: FileLike,
    Log: SchemaEvent<Uuid, F>,
{
    type F = F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.keys()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&F> {
        self.get(id)
    }
}

impl<F, Log> TreeLikeMut for TransactionTable<'_, Uuid, F, Log>
where
    F: FileLike,
    Log: SchemaEvent<Uuid, F>,
{
    fn insert(&mut self, f: F) -> Option<F> {
        TransactionTable::insert(self, *f.id(), f)
    }

    fn remove(&mut self, id: Uuid) -> Option<F> {
        self.delete(id)
    }

    fn clear(&mut self) {
        self.clear()
    }
}
