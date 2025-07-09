use crate::model::file_like::FileLike;
use crate::model::lazy::LazyTree;
use crate::model::staged::StagedTree;
use std::collections::HashMap;
use std::fmt::Debug;
use uuid::Uuid;

use super::errors::{LbErrKind, LbResult};

pub trait TreeLike: Sized {
    type F: FileLike + Debug;

    // todo: iterator using const generics
    fn ids(&self) -> Vec<Uuid>;
    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F>;

    fn find(&self, id: &Uuid) -> LbResult<&Self::F> {
        self.maybe_find(id)
            .ok_or_else(|| LbErrKind::FileNonexistent.into())
    }

    fn maybe_find_parent<F2: FileLike>(&self, file: &F2) -> Option<&Self::F> {
        self.maybe_find(file.parent())
    }

    fn find_parent<F2: FileLike>(&self, file: &F2) -> LbResult<&Self::F> {
        self.maybe_find_parent(file)
            .ok_or_else(|| LbErrKind::FileParentNonexistent.into())
    }

    fn all_files(&self) -> LbResult<Vec<&Self::F>> {
        let ids = self.ids();
        let mut all = Vec::with_capacity(ids.len());
        for id in ids {
            let meta = self.find(&id)?;
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
    fn insert(&mut self, f: Self::F) -> LbResult<Option<Self::F>>;
    fn remove(&mut self, id: Uuid) -> LbResult<Option<Self::F>>;
    fn clear(&mut self) -> LbResult<()>;
}

impl<T> TreeLike for &T
where
    T: TreeLike,
{
    type F = T::F;

    fn ids(&self) -> Vec<Uuid> {
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

    fn ids(&self) -> Vec<Uuid> {
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
    fn insert(&mut self, f: Self::F) -> LbResult<Option<Self::F>> {
        T::insert(self, f)
    }

    fn remove(&mut self, id: Uuid) -> LbResult<Option<Self::F>> {
        T::remove(self, id)
    }

    fn clear(&mut self) -> LbResult<()> {
        T::clear(self)
    }
}

impl<F> TreeLike for Vec<F>
where
    F: FileLike,
{
    type F = F;

    fn ids(&self) -> Vec<Uuid> {
        self.iter().map(|f| *f.id()).collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&F> {
        self.iter().find(|f| f.id() == id)
    }
}

impl<F> TreeLikeMut for Vec<F>
where
    F: FileLike,
{
    fn insert(&mut self, f: F) -> LbResult<Option<F>> {
        for (i, value) in self.iter().enumerate() {
            if value.id() == f.id() {
                let old = std::mem::replace(&mut self[i], f);
                return Ok(Some(old));
            }
        }

        self.push(f);

        Ok(None)
    }

    fn remove(&mut self, id: Uuid) -> LbResult<Option<F>> {
        for (i, value) in self.iter().enumerate() {
            if *value.id() == id {
                return Ok(Some(self.remove(i)));
            }
        }

        Ok(None)
    }

    fn clear(&mut self) -> LbResult<()> {
        self.clear();
        Ok(())
    }
}

impl<F> TreeLike for HashMap<Uuid, F>
where
    F: FileLike,
{
    type F = F;

    fn ids(&self) -> Vec<Uuid> {
        self.keys().copied().collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&F> {
        self.get(id)
    }
}

impl<F> TreeLikeMut for HashMap<Uuid, F>
where
    F: FileLike,
{
    fn insert(&mut self, f: F) -> LbResult<Option<F>> {
        Ok(self.insert(*f.id(), f))
    }

    fn remove(&mut self, id: Uuid) -> LbResult<Option<F>> {
        Ok(self.remove(&id))
    }

    fn clear(&mut self) -> LbResult<()> {
        self.clear();
        Ok(())
    }
}
