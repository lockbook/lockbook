use crate::file::like::FileLike;
use crate::tree::like::{TreeLike, TreeLikeMut};
use crate::tree::stagable::{Stagable, StagableMut};
use std::collections::HashSet;
use uuid::Uuid;

pub trait StagedTreeLike: TreeLike {
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

impl<T> StagedTreeLike for &T
where
    T: StagedTreeLike,
{
    type Base = T::Base;
    type Staged = T::Staged;

    fn base(&self) -> &Self::Base {
        T::base(self)
    }

    fn staged(&self) -> &Self::Staged {
        T::staged(self)
    }
}

impl<T> StagedTreeLike for &mut T
where
    T: StagedTreeLike,
{
    type Base = T::Base;
    type Staged = T::Staged;

    fn base(&self) -> &Self::Base {
        T::base(self)
    }

    fn staged(&self) -> &Self::Staged {
        T::staged(self)
    }
}

// todo: make this trait not generic once associated type bounds are stabilized
// https://rust-lang.github.io/rfcs/2289-associated-type-bounds.html
pub trait StagedTreeLikeMut<Base, Staged>:
    StagedTreeLike<Base = Base, Staged = Staged> + TreeLikeMut
where
    Base: StagableMut<F = Self::F>,
    Staged: StagableMut<F = Self::F>,
{
    fn base_mut(&mut self) -> &mut Self::Base;
    fn staged_mut(&mut self) -> &mut Self::Staged;

    fn prune(&mut self) {
        let mut prunable = vec![];
        for id in self.staged().ids() {
            if let Some(staged) = self.staged().maybe_find(id) {
                if let Some(base) = self.base().maybe_find(id) {
                    if staged == base {
                        prunable.push(*id);
                    }
                }
            }
        }

        for id in prunable {
            self.staged_mut().remove(id);
        }
    }
}

impl<T, Base, Staged> StagedTreeLikeMut<Base, Staged> for &mut T
where
    T: StagedTreeLikeMut<Base, Staged>,
    Staged: StagableMut<F = Self::F>,
    Base: StagableMut<F = Self::F>,
{
    fn base_mut(&mut self) -> &mut Self::Base {
        T::base_mut(self)
    }

    fn staged_mut(&mut self) -> &mut Self::Staged {
        T::staged_mut(self)
    }
}

#[derive(Debug)]
pub struct StagedTreeRef<'b, 's, Base, Staged>
where
    Base: Stagable,
    Staged: Stagable<F = Base::F>,
{
    pub base: &'b Base,
    pub staged: &'s Staged,
}

impl<'b, 's, Base, Staged> StagedTreeRef<'b, 's, Base, Staged>
where
    Base: Stagable,
    Staged: Stagable<F = Base::F>,
{
    pub fn new(base: &'b Base, staged: &'s Staged) -> Self {
        Self { base, staged }
    }
}

impl<Base, Staged> TreeLike for StagedTreeRef<'_, '_, Base, Staged>
where
    Base: Stagable,
    Staged: Stagable<F = Base::F>,
{
    type F = Base::F;

    fn ids(&self) -> HashSet<&Uuid> {
        StagedTreeLike::ids(self)
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        StagedTreeLike::maybe_find(self, id)
    }
}

impl<Base, Staged> StagedTreeLike for StagedTreeRef<'_, '_, Base, Staged>
where
    Base: Stagable,
    Staged: Stagable<F = Base::F>,
{
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
pub struct StagedTree<Base, Staged>
where
    Base: StagableMut,
    Staged: StagableMut<F = Base::F>,
{
    pub base: Base,
    pub staged: Staged,
}

impl<Base: StagableMut, Staged: StagableMut<F = Base::F>> StagedTree<Base, Staged> {
    pub fn new(base: Base, staged: Staged) -> Self {
        let mut result = Self { base, staged };
        result.prune();
        result
    }
}

impl<Base: StagableMut, Staged: StagableMut<F = Base::F>> TreeLike for StagedTree<Base, Staged> {
    type F = Base::F;

    fn ids(&self) -> HashSet<&Uuid> {
        StagedTreeLike::ids(self)
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        StagedTreeLike::maybe_find(self, id)
    }
}

impl<Base: StagableMut, Staged: StagableMut<F = Base::F>> TreeLikeMut for StagedTree<Base, Staged> {
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

impl<Base, Staged> StagedTreeLike for StagedTree<Base, Staged>
where
    Base: StagableMut,
    Staged: StagableMut<F = Base::F>,
{
    type Base = Base;
    type Staged = Staged;

    fn base(&self) -> &Self::Base {
        &self.base
    }

    fn staged(&self) -> &Self::Staged {
        &self.staged
    }
}

impl<'s, Base: StagableMut, Staged: StagableMut<F = Base::F>> StagedTreeLikeMut<Base, Staged>
    for StagedTree<Base, Staged>
{
    fn base_mut(&mut self) -> &mut Self::Base {
        &mut self.base
    }

    fn staged_mut(&mut self) -> &mut Self::Staged {
        &mut self.staged
    }
}
