use crate::model::file_like::FileLike;
use crate::model::tree_like::{TreeLike, TreeLikeMut};
use crate::model::SharedResult;
use std::collections::HashSet;
use uuid::Uuid;

use super::errors::LbResult;

pub trait StagedTreeLike: TreeLike {
    type Base: TreeLike<F = Self::F>;
    type Staged: TreeLike<F = Self::F>;

    fn base(&self) -> &Self::Base;
    fn staged(&self) -> &Self::Staged;
}

// todo: make this trait not generic once associated type bounds are stabilized
// https://rust-lang.github.io/rfcs/2289-associated-type-bounds.html
pub trait StagedTreeLikeMut<Base, Staged>:
    StagedTreeLike<Base = Base, Staged = Staged> + TreeLikeMut
where
    Base: TreeLike<F = Self::F>,
    Staged: TreeLikeMut<F = Self::F>,
{
    fn staged_mut(&mut self) -> &mut Self::Staged;

    fn prune(&mut self) -> LbResult<()> {
        let mut prunable = vec![];
        for id in self.staged().ids() {
            if let Some(staged) = self.staged().maybe_find(&id) {
                if let Some(base) = self.base().maybe_find(&id) {
                    if staged == base {
                        prunable.push(id);
                    }
                }
            }
        }

        for id in prunable {
            self.staged_mut().remove(id)?;
        }
        Ok(())
    }

    fn pruned(mut self) -> LbResult<Self> {
        self.prune()?;
        Ok(self)
    }
}

#[derive(Clone, Debug)]
pub struct StagedTree<Base, Staged>
where
    Base: TreeLike,
    Staged: TreeLike<F = Base::F>,
{
    pub base: Base,
    pub staged: Staged,
    pub new: HashSet<Uuid>,
    pub removed: HashSet<Uuid>,
}

impl<Base, Staged> StagedTree<Base, Staged>
where
    Base: TreeLike,
    Staged: TreeLike<F = Base::F>,
{
    pub fn new(base: Base, staged: Staged) -> Self {
        let mut new = HashSet::new();
        for id in staged.ids() {
            if base.maybe_find(&id).is_none() {
                new.insert(id);
            }
        }
        Self { base, staged, removed: HashSet::new(), new }
    }
}

impl<Base> StagedTree<Base, Option<Base::F>>
where
    Base: TreeLike,
{
    // todo: this is dead code
    pub fn removal(base: Base, removed: HashSet<Uuid>) -> Self {
        Self { base, staged: None, removed, new: Default::default() }
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

impl<Base, Staged, T> StagedTreeLikeMut<Base, Staged> for &mut T
where
    Base: TreeLike<F = T::F>,
    Staged: TreeLikeMut<F = T::F>,
    T: StagedTreeLikeMut<Base, Staged>,
{
    fn staged_mut(&mut self) -> &mut Self::Staged {
        T::staged_mut(self)
    }
}

impl<Base, Staged> TreeLike for StagedTree<Base, Staged>
where
    Base: TreeLike,
    Staged: TreeLike<F = Base::F>,
{
    type F = Base::F;

    fn ids(&self) -> Vec<Uuid> {
        let mut all_ids = self.base.ids();
        all_ids.extend(&self.new);
        all_ids.retain(|id| !self.removed.contains(id));

        all_ids
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F> {
        if self.removed.contains(id) {
            None
        } else {
            self.staged()
                .maybe_find(id)
                .or_else(|| self.base().maybe_find(id))
        }
    }
}

impl<Base, Staged> TreeLikeMut for StagedTree<Base, Staged>
where
    Base: TreeLike,
    Staged: TreeLikeMut<F = Base::F>,
{
    fn insert(&mut self, f: Self::F) -> LbResult<Option<Self::F>> {
        // if we're inserting it, it can't be removed
        self.removed.remove(f.id());

        if let Some(base) = self.base.maybe_find(f.id()) {
            if *base == f {
                return self.staged.remove(*f.id());
            }
        } else {
            self.new.insert(*f.id());
        }

        self.staged.insert(f)
    }

    fn remove(&mut self, id: Uuid) -> LbResult<Option<Self::F>> {
        self.removed.insert(id);
        if let Some(staged) = self.staged.remove(id)? {
            Ok(Some(staged))
        } else {
            Ok(self.base.maybe_find(&id).cloned())
        }
    }

    fn clear(&mut self) -> LbResult<()> {
        self.removed.extend(self.ids());
        Ok(())
    }
}

impl<Base, Staged> StagedTreeLike for StagedTree<Base, Staged>
where
    Base: TreeLike,
    Staged: TreeLike<F = Base::F>,
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
    Base: TreeLike,
    Staged: TreeLikeMut<F = Base::F>,
{
    fn staged_mut(&mut self) -> &mut Self::Staged {
        &mut self.staged
    }
}
