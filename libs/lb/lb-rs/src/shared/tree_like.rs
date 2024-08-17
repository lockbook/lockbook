use crate::shared::file_like::FileLike;
use crate::shared::lazy::LazyTree;
use crate::shared::staged::StagedTree;
use crate::shared::{SharedErrorKind, SharedResult};
use std::collections::HashSet;
use std::fmt::Debug;
use uuid::Uuid;

pub trait TreeLike: Sized {
    type F: FileLike + Debug;

    // todo: iterator using const generics
    fn ids(&self) -> HashSet<&Uuid>;
    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F>;

    fn find(&self, id: &Uuid) -> SharedResult<&Self::F> {
        self.maybe_find(id)
            .ok_or_else(|| SharedErrorKind::FileNonexistent.into())
    }

    fn maybe_find_parent<F2: FileLike>(&self, file: &F2) -> Option<&Self::F> {
        self.maybe_find(file.parent())
    }

    fn find_parent<F2: FileLike>(&self, file: &F2) -> SharedResult<&Self::F> {
        self.maybe_find_parent(file)
            .ok_or_else(|| SharedErrorKind::FileParentNonexistent.into())
    }

    fn owned_ids(&self) -> HashSet<Uuid> {
        self.ids().iter().map(|id| **id).collect()
    }

    fn all_files(&self) -> SharedResult<Vec<&Self::F>> {
        let mut all = vec![];
        for id in self.ids() {
            let meta = self.find(id)?;
            all.push(meta);
        }

        Ok(all)
    }

    fn stage<Staged>(&self, staged: Staged) -> StagedTree<&Self, Staged>
    where
        Staged: TreeLike<F = Self::F>,
        Self: Sized,
    {
        StagedTree::new(self, staged)
    }

    fn to_staged<Staged>(self, staged: Staged) -> StagedTree<Self, Staged>
    where
        Staged: TreeLike<F = Self::F>,
        Self: Sized,
    {
        StagedTree::new(self, staged)
    }

    fn as_lazy(&self) -> LazyTree<&Self> {
        LazyTree::new(self)
    }

    fn to_lazy(self) -> LazyTree<Self> {
        LazyTree::new(self)
    }
}

pub trait TreeLikeMut: TreeLike {
    fn insert(&mut self, f: Self::F) -> SharedResult<Option<Self::F>>;
    fn remove(&mut self, id: Uuid) -> SharedResult<Option<Self::F>>;
    fn clear(&mut self) -> SharedResult<()>;
}

impl<T> TreeLike for &T
where
    T: TreeLike,
{
    type F = T::F;

    fn ids(&self) -> HashSet<&Uuid> {
        T::ids(self)
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        T::maybe_find(self, id)
    }
}

impl<T> TreeLike for &mut T
where
    T: TreeLike,
{
    type F = T::F;

    fn ids(&self) -> HashSet<&Uuid> {
        T::ids(self)
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        T::maybe_find(self, id)
    }
}

impl<T> TreeLikeMut for &mut T
where
    T: TreeLikeMut,
{
    fn insert(&mut self, f: Self::F) -> SharedResult<Option<Self::F>> {
        T::insert(self, f)
    }

    fn remove(&mut self, id: Uuid) -> SharedResult<Option<Self::F>> {
        T::remove(self, id)
    }

    fn clear(&mut self) -> SharedResult<()> {
        T::clear(self)
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
}

impl<F> TreeLikeMut for Vec<F>
where
    F: FileLike,
{
    fn insert(&mut self, f: F) -> SharedResult<Option<F>> {
        for (i, value) in self.iter().enumerate() {
            if value.id() == f.id() {
                let old = std::mem::replace(&mut self[i], f);
                return Ok(Some(old));
            }
        }

        self.push(f);

        Ok(None)
    }

    fn remove(&mut self, id: Uuid) -> SharedResult<Option<F>> {
        for (i, value) in self.iter().enumerate() {
            if *value.id() == id {
                return Ok(Some(self.remove(i)));
            }
        }

        Ok(None)
    }

    fn clear(&mut self) -> SharedResult<()> {
        self.clear();
        Ok(())
    }
}
