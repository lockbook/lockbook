use crate::file_like::FileLike;
use crate::server_file::ServerFile;
use crate::signed_file::SignedFile;
use crate::staged::StagedTree;
use crate::tree_like::TreeError::*;
use std::collections::HashSet;
use uuid::Uuid;

pub enum ValidationError {
    Todo,
}

pub trait TreeLike<F: FileLike> {
    fn ids(&self) -> HashSet<Uuid>;
    fn maybe_find(&self, id: Uuid) -> Option<&F>;

    fn find(&self, id: Uuid) -> Result<&F, TreeError> {
        self.maybe_find(id).ok_or(FileNonexistent)
    }

    fn maybe_find_parent<F2: FileLike>(&self, file: &F2) -> Option<&F> {
        self.maybe_find(file.parent())
    }

    fn find_parent<F2: FileLike>(&self, file: &F2) -> Result<&F, TreeError> {
        self.maybe_find_parent(file).ok_or(FileParentNonexistent)
    }

    fn stage<'a, Staged>(&'a self, staged: &'a Staged) -> StagedTree<'a, F, Self, Staged>
    where
        Staged: TreeLike<F>,
        Self: Sized,
    {
        StagedTree::new(self, staged)
    }

    fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
}

impl<F: FileLike> TreeLike<F> for Vec<F> {
    fn ids(&self) -> HashSet<Uuid> {
        self.iter().map(|f| f.id()).collect()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&F> {
        self.iter().find(|f| f.id() == id)
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum TreeError {
    RootNonexistent,
    FileNonexistent,
    FileParentNonexistent,
    Unexpected(String),
}
