use crate::file::like::FileLike;
use crate::tree::like::{TreeLike, TreeLikeMut};
use crate::tree::stagable::Stagable;
use std::collections::HashSet;
use uuid::Uuid;

pub trait StagedTreeLike: Sized {
    type F: FileLike;
    type Base: Stagable<F = Self::F>;
    type Staged: Stagable<F = Self::F>;

    fn base(&self) -> &Self::Base;
    fn staged(&self) -> &Self::Staged;

    fn ids(&self) -> HashSet<&Uuid> {
        self.base()
            .ids()
            .into_iter()
            .chain(self.staged().ids().into_iter())
            .collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        self.staged()
            .maybe_find(id)
            .or_else(|| self.base().maybe_find(id))
    }
}

// todo: ??
pub trait StagedTreeLikeMut: StagedTreeLike {}

pub struct StagedTreeRef<'b, 's, Base, Staged>
where
    Base: Stagable,
    Staged: Stagable<F = Base::F>,
{
    pub base: &'b Base,
    pub staged: &'s Staged,
}

impl<Base, Staged> StagedTreeLike for StagedTreeRef<'_, '_, Base, Staged>
where
    Base: Stagable,
    Staged: Stagable<F = Base::F>,
{
    type F = Base::F;
    type Base = Base;
    type Staged = Staged;

    fn base(&self) -> &Self::Base {
        self.base
    }

    fn staged(&self) -> &Self::Staged {
        self.staged
    }
}

#[derive(Debug)]
pub struct StagedTree<'s, Base, Staged>
where
    Base: Stagable,
    Staged: Stagable<F = Base::F>,
{
    pub base: Base,
    pub staged: &'s mut Staged,
}

impl<Base, Staged> StagedTreeLike for StagedTree<'_, Base, Staged>
where
    Base: Stagable,
    Staged: Stagable<F = Base::F>,
{
    type F = Base::F;
    type Base = Base;
    type Staged = Staged;

    fn base(&self) -> &Self::Base {
        &self.base
    }

    fn staged(&self) -> &Self::Staged {
        self.staged
    }
}

impl<'s, Base: Stagable, Staged: Stagable<F = Base::F>> StagedTree<'s, Base, Staged> {
    pub fn new(base: Base, staged: &'s mut Staged) -> Self {
        let mut prunable = vec![];
        for id in staged.ids() {
            if let Some(staged) = staged.maybe_find(id) {
                if let Some(base) = base.maybe_find(id) {
                    if staged == base {
                        prunable.push(*id);
                    }
                }
            }
        }

        for id in prunable {
            staged.remove(id);
        }
        Self { base, staged }
    }
}

impl<'s, Base: Stagable, Staged: Stagable<F = Base::F>> TreeLike for StagedTree<'s, Base, Staged> {
    type F = Base::F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.base
            .ids()
            .into_iter()
            .chain(self.staged.ids().into_iter())
            .collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        match (self.base.maybe_find(id), self.staged.maybe_find(id)) {
            (_, Some(staged)) => Some(staged),
            (Some(base), None) => Some(base),
            (None, None) => None,
        }
    }
}

impl<'s, Base: Stagable, Staged: Stagable<F = Base::F>> TreeLikeMut
    for StagedTree<'s, Base, Staged>
{
    fn insert(&mut self, f: Self::F) -> Option<Self::F> {
        if let Some(base) = self.base.maybe_find(f.id()) {
            if *base == f {
                return self.staged.remove(*f.id());
            }
        }

        self.staged.insert(f)
    }

    fn remove(&mut self, id: Uuid) -> Option<Self::F> {
        match (self.base.remove(id), self.staged.remove(id)) {
            (_, Some(staged)) => Some(staged),
            (Some(base), None) => Some(base),
            (None, None) => None,
        }
    }
}

impl<'s, Base: Stagable, Staged: Stagable<F = Base::F>> Stagable for StagedTree<'s, Base, Staged> {}

impl<F> Stagable for Option<F> where F: FileLike {}

impl<F: FileLike> Stagable for Vec<F> {}
