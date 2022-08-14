use crate::file_like::FileLike;
use crate::tree_like::{Stagable, TreeLike};
use hmdb::log::SchemaEvent;
use hmdb::transaction::TransactionTable;
use std::collections::HashSet;
use uuid::Uuid;

impl<'a, F, Log> TreeLike for &mut TransactionTable<'a, Uuid, F, Log>
where
    F: FileLike + Clone,
    Log: SchemaEvent<Uuid, F>,
{
    type F = F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.keys()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&F> {
        self.get(id)
    }

    fn insert(&mut self, f: F) -> Option<F> {
        TransactionTable::insert(self, *f.id(), f)
    }

    fn remove(&mut self, id: Uuid) -> Option<F> {
        self.delete(id)
    }
}

impl<'a, F, Log> Stagable for &mut TransactionTable<'a, Uuid, F, Log>
where
    F: FileLike + Clone,
    Log: SchemaEvent<Uuid, F>,
{
}
