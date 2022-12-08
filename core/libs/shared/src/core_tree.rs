use crate::file_like::FileLike;
use crate::tree_like::{TreeLike, TreeLikeMut};
use hmdb::log::SchemaEvent;
use hmdb::transaction::TransactionTable;
use std::collections::HashSet;
use uuid::Uuid;

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
