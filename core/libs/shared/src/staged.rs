use crate::file_like::FileLike;
use crate::tree_like::{TreeLike, TreeLikeMut};
use std::collections::HashSet;
use uuid::Uuid;

pub trait StagedTreeLike: TreeLike {
    type Base: TreeLike<F = Self::F>;
    type Staged: TreeLike<F = Self::F>;

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

// todo: make this trait not generic once associated type bounds are stabilized
// https://rust-lang.github.io/rfcs/2289-associated-type-bounds.html
pub trait StagedTreeLikeMut<Base, Staged>:
    StagedTreeLike<Base = Base, Staged = Staged> + TreeLikeMut
where
    Base: TreeLikeMut<F = Self::F>,
    Staged: TreeLikeMut<F = Self::F>,
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

#[derive(Debug)]
pub struct StagedTreeRef<'b, 's, Base, Staged>
where
    Base: TreeLike,
    Staged: TreeLike<F = Base::F>,
{
    pub base: &'b Base,
    pub staged: &'s Staged,
}

impl<'b, 's, Base, Staged> StagedTreeRef<'b, 's, Base, Staged>
where
    Base: TreeLikeMut,
    Staged: TreeLikeMut<F = Base::F>,
{
    pub fn new(base: &'b Base, staged: &'s Staged) -> Self {
        Self { base, staged }
    }
}

impl<Base, Staged> TreeLike for StagedTreeRef<'_, '_, Base, Staged>
where
    Base: TreeLike,
    Staged: TreeLike<F = Base::F>,
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
    Base: TreeLike,
    Staged: TreeLike<F = Base::F>,
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
    Base: TreeLikeMut,
    Staged: TreeLikeMut<F = Base::F>,
{
    pub base: Base,
    pub staged: Staged,
}

impl<Base, Staged> StagedTree<Base, Staged>
where
    Base: TreeLikeMut,
    Staged: TreeLikeMut<F = Base::F>,
{
    pub fn new(base: Base, mut staged: Staged) -> Self {
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

impl<Base, Staged> TreeLike for StagedTree<Base, Staged>
where
    Base: TreeLikeMut,
    Staged: TreeLikeMut<F = Base::F>,
{
    type F = Base::F;

    fn ids(&self) -> HashSet<&Uuid> {
        StagedTreeLike::ids(self)
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        StagedTreeLike::maybe_find(self, id)
    }
}

impl<Base, Staged> TreeLikeMut for StagedTree<Base, Staged>
where
    Base: TreeLikeMut,
    Staged: TreeLikeMut<F = Base::F>,
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

impl<Base, Staged> StagedTreeLike for StagedTree<Base, Staged>
where
    Base: TreeLikeMut,
    Staged: TreeLikeMut<F = Base::F>,
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

impl<Base, Staged> StagedTreeLikeMut<Base, Staged> for StagedTree<Base, Staged>
where
    Base: TreeLikeMut,
    Staged: TreeLikeMut<F = Base::F>,
{
    fn base_mut(&mut self) -> &mut Self::Base {
        &mut self.base
    }

    fn staged_mut(&mut self) -> &mut Self::Staged {
        &mut self.staged
    }
}
