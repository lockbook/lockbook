use crate::file_like::FileLike;
use crate::tree_like::{TreeLike, TreeLikeMut};
use crate::SharedResult;
use hmdb::log::SchemaEvent;
use hmdb::transaction::TransactionTable;
use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

impl<F> TreeLike for db_rs::LookupTable<Uuid, F>
where
    F: FileLike + Serialize,
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
    fn insert(&mut self, f: F) -> SharedResult<Option<F>> {
        Ok(TransactionTable::insert(self, *f.id(), f))
    }

    fn remove(&mut self, id: Uuid) -> SharedResult<Option<F>> {
        Ok(self.delete(id))
    }

    fn clear(&mut self) -> SharedResult<()> {
        self.clear();
        Ok(())
    }
}
