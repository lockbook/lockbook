use crate::file::like::FileLike;
use crate::{SharedError, SharedResult};
use std::collections::HashSet;
use std::fmt::Debug;
use uuid::Uuid;

pub trait TreeLike: Sized {
    type F: FileLike + Debug;

    // todo: iterator
    fn ids(&self) -> HashSet<&Uuid>;
    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F>;

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
        let mut all = vec![];
        for id in self.ids() {
            let meta = self.find(id)?;
            all.push(meta);
        }

        Ok(all)
    }
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

pub trait TreeLikeMut: TreeLike {
    fn insert(&mut self, f: Self::F) -> Option<Self::F>;
    fn remove(&mut self, id: Uuid) -> Option<Self::F>;
}

impl<T> TreeLikeMut for &mut T
where
    T: TreeLikeMut,
{
    fn insert(&mut self, f: Self::F) -> Option<Self::F> {
        T::insert(self, f)
    }

    fn remove(&mut self, id: Uuid) -> Option<Self::F> {
        T::remove(self, id)
    }
}

impl<F> TreeLike for Option<F>
where
    F: FileLike,
{
    type F = F;

    fn ids(&self) -> HashSet<&Uuid> {
        let mut hashset = HashSet::new();
        if let Some(f) = self {
            hashset.insert(f.id());
        }
        hashset
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&F> {
        if let Some(f) = self {
            if id == f.id() {
                self.as_ref()
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl<F> TreeLikeMut for Option<F>
where
    F: FileLike,
{
    fn insert(&mut self, f: F) -> Option<F> {
        self.replace(f)
    }

    fn remove(&mut self, id: Uuid) -> Option<F> {
        if let Some(f) = self {
            if &id == f.id() {
                self.take()
            } else {
                None
            }
        } else {
            None
        }
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
    fn insert(&mut self, f: F) -> Option<F> {
        for (i, value) in self.iter().enumerate() {
            if value.id() == f.id() {
                let old = std::mem::replace(&mut self[i], f);
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
