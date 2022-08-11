use crate::file_like::FileLike;
use crate::file_metadata::Owner;
use crate::lazy::{LazyStage2, LazyStaged1, LazyTree};
use crate::signed_file::SignedFile;
use crate::tree_like::{Stagable, TreeLike};
use hmdb::log::SchemaEvent;
use hmdb::transaction::TransactionTable;
use std::collections::HashSet;
use tracing::error;
use uuid::Uuid;
extern crate tracing;

pub struct CoreTree<'a, 'b, Log>
where
    Log: SchemaEvent<Uuid, SignedFile>,
{
    pub owner: Owner,
    pub metas: &'a mut TransactionTable<'b, Uuid, SignedFile, Log>,
}

impl<'a, 'b, Log> LazyTree<CoreTree<'a, 'b, Log>>
where
    Log: SchemaEvent<Uuid, SignedFile>,
{
    pub fn base_tree(
        owner: Owner, base: &'a mut TransactionTable<'b, Uuid, SignedFile, Log>,
    ) -> Self {
        CoreTree { owner, metas: base }.to_lazy()
    }
}

impl<'a, 'b, Log1, Log2> LazyStaged1<CoreTree<'a, 'b, Log1>, CoreTree<'a, 'b, Log2>>
where
    Log1: SchemaEvent<Uuid, SignedFile>,
    Log2: SchemaEvent<Uuid, SignedFile>,
{
    pub fn core_tree(
        owner: Owner, base: &'a mut TransactionTable<'b, Uuid, SignedFile, Log1>,
        local_changes: &'a mut TransactionTable<'b, Uuid, SignedFile, Log2>,
    ) -> Self {
        CoreTree { owner, metas: base }
            .stage(CoreTree { owner, metas: local_changes })
            .to_lazy()
    }
}

impl<'a, 'b, Log1, Log2, Remote> LazyStage2<CoreTree<'a, 'b, Log1>, Remote, CoreTree<'a, 'b, Log2>>
where
    Log1: SchemaEvent<Uuid, SignedFile>,
    Remote: Stagable<F = SignedFile>,
    Log2: SchemaEvent<Uuid, SignedFile>,
{
    pub fn core_tree_with_remote(
        owner: Owner, base: &'a mut TransactionTable<'b, Uuid, SignedFile, Log1>,
        remote_changes: Remote,
        local_changes: &'a mut TransactionTable<'b, Uuid, SignedFile, Log2>,
    ) -> Self {
        CoreTree { owner, metas: base }
            .stage(remote_changes)
            .stage(CoreTree { owner, metas: local_changes })
            .to_lazy()
    }
}

impl<'a, 'b, Log> TreeLike for CoreTree<'a, 'b, Log>
where
    Log: SchemaEvent<Uuid, SignedFile>,
{
    type F = SignedFile;

    fn ids(&self) -> HashSet<&Uuid> {
        self.metas
            .get_all()
            .values()
            .filter_map(|file| if file.owner() == self.owner { Some(file.id()) } else { None })
            .collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        self.metas
            .get(id)
            .and_then(|f| if f.owner() == self.owner { Some(f) } else { None })
    }

    fn insert(&mut self, f: Self::F) -> Option<Self::F> {
        if f.owner() != self.owner {
            error!("core tree file insert owner mismatch")
        }
        TransactionTable::insert(self.metas, *f.id(), f)
    }

    fn remove(&mut self, id: Uuid) -> Option<Self::F> {
        let maybe_f = self.metas.delete(id);
        if let Some(f) = &maybe_f {
            if f.owner() != self.owner {
                error!("core tree file insert owner mismatch")
            }
        }
        maybe_f
    }
}

impl<'a, 'b, Log> Stagable for CoreTree<'a, 'b, Log> where Log: SchemaEvent<Uuid, SignedFile> {}
