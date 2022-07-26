use crate::file_like::FileLike;
use crate::lazy::LazyTree;
use crate::server_file::ServerFile;
use crate::signed_file::SignedFile;
use crate::staged::StagedTree;
use crate::{SharedError, SharedResult};
use std::collections::HashSet;
use uuid::Uuid;

pub trait TreeLike: Sized {
    type F: FileLike;

    fn ids(&self) -> HashSet<&Uuid>;
    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F>;
    fn insert(&mut self, f: Self::F) -> Option<Self::F>;
    fn remove(&mut self, id: Uuid) -> Option<Self::F>;

    fn find(&self, id: &Uuid) -> SharedResult<&Self::F> {
        self.maybe_find(id).ok_or(SharedError::FileNonexistent)
    }

    fn maybe_find_parent<F2: FileLike>(&self, file: &F2) -> Option<&Self::F> {
        self.maybe_find(file.parent())
    }

    fn find_parent<F2: FileLike>(&self, file: &F2) -> SharedResult<&Self::F> {
        self.maybe_find_parent(file)
            .ok_or(SharedError::FileParentNonexistent)
    }

    fn owned_ids(&self) -> HashSet<Uuid> {
        self.ids().iter().map(|id| **id).collect()
    }

    fn all_files(&self) -> SharedResult<Vec<&Self::F>> {
        todo!()
    }
}

pub trait Stagable: TreeLike {
    fn stage<Staged>(self, staged: Staged) -> StagedTree<Self, Staged>
    where
        Staged: Stagable<F = Self::F>,
        Self: Sized,
    {
        StagedTree::new(self, staged)
    }

    fn to_lazy(self) -> LazyTree<Self> {
        LazyTree::new(self)
    }
}

impl<F> TreeLike for Vec<F>
where
    F: FileLike,
{
    type F = F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.iter().map(|f| f.id()).collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&F> {
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
            if *value.id() == id {
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
