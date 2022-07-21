use crate::{RequestContext, Tx};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::lazy::{LazyStaged1, LazyTree};
use lockbook_shared::server_file::ServerFile;
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::staged::StagedTree;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use std::collections::HashSet;
use uuid::Uuid;

struct Base<'a>(&'a mut Tx<'a>);
struct Local<'a>(&'a mut Tx<'a>);

impl TreeLike<ServerFile> for Base<'_> {
    fn ids(&self) -> HashSet<Uuid> {
        todo!()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&ServerFile> {
        self.0.base_metadata.get(&id)
    }

    fn insert(&mut self, f: ServerFile) -> Option<ServerFile> {
        self.0.base_metadata.insert(f.id(), f)
    }

    fn remove(&mut self, id: Uuid) -> Option<ServerFile> {
        self.0.base_metadata.delete(id)
    }
}

impl<'a> Stagable<ServerFile> for Base<'a> {}

impl TreeLike<SignedFile> for Local<'_> {
    fn ids(&self) -> HashSet<Uuid> {
        todo!()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&SignedFile> {
        self.0.local_metadata.get(&id)
    }

    fn insert(&mut self, f: SignedFile) -> Option<SignedFile> {
        self.0.local_metadata.insert(f.id(), f)
    }

    fn remove(&mut self, id: Uuid) -> Option<SignedFile> {
        self.0.local_metadata.delete(id)
    }
}

impl<'a> Stagable<SignedFile> for Local<'a> {}

pub type CoreTree<'a> = LazyStaged1<SignedFile, Base<'a>, Local<'a>>;

struct StagedDbFiles<'a> {
    base: Base<'a>,
    local: Local<'a>,
}

impl RequestContext<'_, '_> {
    pub fn tree(&self) -> CoreTree {
        Base(self.tx).stage(Local(self.tx)).lazy()
    }
}
