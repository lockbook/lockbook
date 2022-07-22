use crate::file_like::FileLike;
use crate::tree_like::TreeLike;
use hmdb::log::SchemaEvent;
use hmdb::transaction::TransactionTable;
use std::collections::HashSet;
use uuid::Uuid;

impl<'a, F, Log> TreeLike for TransactionTable<'a, Uuid, F, Log>
where
    F: FileLike + Clone,
    Log: SchemaEvent<Uuid, F>,
{
    type F = F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.keys()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&F> {
        todo!()
    }

    fn insert(&mut self, f: F) -> Option<F> {
        todo!()
    }

    fn remove(&mut self, id: Uuid) -> Option<F> {
        todo!()
    }
}
