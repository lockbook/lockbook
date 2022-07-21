use crate::file_like::FileLike;
use crate::lazy::LazyTree;
use crate::server_file::ServerFile;
use crate::signed_file::SignedFile;
use crate::staged::StagedTree;
use crate::{SharedError, SharedResult};
use std::collections::HashSet;
use uuid::Uuid;

pub trait TreeLike<F: FileLike>: Sized {
    fn ids(&self) -> HashSet<Uuid>;
    fn maybe_find(&self, id: Uuid) -> Option<&F>;
    fn insert(&mut self, f: F) -> Option<F>;
    fn remove(&mut self, id: Uuid) -> Option<F>;

    fn find(&self, id: Uuid) -> SharedResult<&F> {
        self.maybe_find(id).ok_or(SharedError::FileNonexistent)
    }

    fn maybe_find_parent<F2: FileLike>(&self, file: &F2) -> Option<&F> {
        self.maybe_find(file.parent())
    }

    fn find_parent<F2: FileLike>(&self, file: &F2) -> SharedResult<&F> {
        self.maybe_find_parent(file)
            .ok_or(SharedError::FileParentNonexistent)
    }
}

pub trait Stagable<F: FileLike>: TreeLike<F> {
    fn stage<Staged>(self, staged: Staged) -> StagedTree<F, Self, Staged>
    where
        Staged: Stagable<F>,
        Self: Sized,
    {
        StagedTree::new(self, staged)
    }

    fn to_lazy(self) -> LazyTree<F, Self> {
        LazyTree::new(self)
    }
}

impl<F: FileLike> TreeLike<F> for Vec<F> {
    fn ids(&self) -> HashSet<Uuid> {
        self.iter().map(|f| f.id()).collect()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&F> {
        self.iter().find(|f| f.id() == id)
    }

    fn insert(&mut self, f: F) -> Option<F> {
        for (i, value) in self.iter().enumerate() {
            if value.id() == f.id() {
                let old = self.remove(i);
                self[i] = f;
                return Some(old);
            }
        }

        self.push(f);

        None
    }

    fn remove(&mut self, id: Uuid) -> Option<F> {
        for (i, value) in self.iter().enumerate() {
            if value.id() == id {
                return Some(self.remove(i));
            }
        }

        None
    }
}

impl<'a> Into<&'a SignedFile> for &'a ServerFile {
    fn into(self) -> &'a SignedFile {
        &self.file
    }
}

impl<T: TreeLike<ServerFile>> TreeLike<SignedFile> for T {
    fn ids(&self) -> HashSet<Uuid> {
        self.ids()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&SignedFile> {
        self.maybe_find(id).map(|f| f.into())
    }

    fn insert(&mut self, f: SignedFile) -> Option<SignedFile> {
        unimplemented!()
    }

    fn remove(&mut self, id: Uuid) -> Option<SignedFile> {
        self.remove(id).map(|f| f.file)
    }
}
