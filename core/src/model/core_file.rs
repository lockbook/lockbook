use crate::Tx;
use hmdb::transaction::TransactionTable;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::server_file::ServerFile;
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::tree_like::{LazyTree, StagedFile, StagedTree, TreeLike};
use std::collections::HashSet;
use uuid::Uuid;

struct Base<'a>(&'a Tx<'a>);
struct Local<'a>(&'a Tx<'a>);

impl TreeLike for Base<'_> {
    type F = ServerFile;

    fn ids(&self) -> HashSet<Uuid> {
        todo!()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&Self::F> {
        self.0.base_metadata.get(&id)
    }
}

impl TreeLike for Local<'_> {
    type F = SignedFile;

    fn ids(&self) -> HashSet<Uuid> {
        todo!()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&Self::F> {
        self.0.local_metadata.get(&id)
    }
}

pub struct StagedChanges<'a> {
    base: Base<'a>,
    local: Local<'a>,
}

impl StagedChanges<'_> {
    fn from_tx(tx: &Tx) -> Self {
        let base = Base(tx);
        let local = Local(tx);

        Self { base, local }
    }
    fn get_tree(&self) -> LazyTree<StagedTree<Base, Local>> {
        LazyTree::new(StagedTree { base: &self.base, local: &self.local })
    }
}
