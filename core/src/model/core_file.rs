use crate::repo::schema::helper_log::{base_metadata, local_metadata};
use crate::{RequestContext, Tx};
use hmdb::transaction::TransactionTable;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::lazy::{LazyStaged1, LazyTree};
use lockbook_shared::server_file::ServerFile;
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::staged::StagedTree;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use std::collections::HashSet;
use uuid::Uuid;

pub struct Base<'a>(pub &'a mut TransactionTable<'a, Uuid, ServerFile, base_metadata>);
pub struct Local<'a>(pub &'a mut TransactionTable<'a, Uuid, SignedFile, local_metadata>);

impl TreeLike for Base<'_> {
    fn ids(&self) -> HashSet<Uuid> {
        todo!()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&ServerFile> {
        self.0.get(&id)
    }

    fn insert(&mut self, f: ServerFile) -> Option<ServerFile> {
        self.0.insert(f.id(), f)
    }

    fn remove(&mut self, id: Uuid) -> Option<ServerFile> {
        self.0.delete(id)
    }
}

impl<'a> Stagable<SignedFile> for Base<'a> {}

impl TreeLike<SignedFile> for Local<'_> {
    fn ids(&self) -> HashSet<Uuid> {
        todo!()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&SignedFile> {
        self.0.get(&id)
    }

    fn insert(&mut self, f: SignedFile) -> Option<SignedFile> {
        self.0.insert(f.id(), f)
    }

    fn remove(&mut self, id: Uuid) -> Option<SignedFile> {
        self.0.delete(id)
    }
}

impl<'a> Stagable<SignedFile> for Local<'a> {}

pub type CoreTree<'a> = LazyStaged1<SignedFile, Base<'a>, Local<'a>>;

impl<'a> RequestContext<'a, 'a> {
    pub fn tree(&'a mut self) -> CoreTree<'a> {
        Base(&mut self.tx.base_metadata)
            .stage(Local(&mut self.tx.local_metadata))
            .to_lazy()
    }
}
