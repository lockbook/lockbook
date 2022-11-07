use crate::file_like::FileLike;
use crate::staged::Stagable;
use crate::tree_like::{TreeLike, TreeLikeMut};
use hmdb::log::SchemaEvent;
use hmdb::transaction::TransactionTable;
use std::collections::HashSet;
use uuid::Uuid;

impl<'a, F, Log> TreeLike for TransactionTable<'a, Uuid, F, Log>
where
    F: FileLike,
    Log: SchemaEvent<Uuid, F>,
{
    type F = F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.keys()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        self.get(id)
    }
}

impl<'a, F, Log> TreeLikeMut for TransactionTable<'a, Uuid, F, Log>
where
    F: FileLike,
    Log: SchemaEvent<Uuid, F>,
{
    fn insert(&mut self, f: Self::F) -> Option<Self::F> {
        TransactionTable::insert(self, *f.id(), f)
    }

    fn remove(&mut self, id: Uuid) -> Option<Self::F> {
        self.delete(id)
    }
}

impl<'a, F, Log> Stagable for TransactionTable<'a, Uuid, F, Log>
where
    F: FileLike,
    Log: SchemaEvent<Uuid, F>,
{
}
