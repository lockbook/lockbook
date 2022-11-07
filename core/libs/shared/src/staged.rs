use crate::file_like::FileLike;
use crate::tree_like::{TreeLike, TreeLikeMut};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug)]
pub struct StagedTree<Base, Staged>
where
    Base: TreeLikeMut,
    Staged: TreeLikeMut<F = Base::F>,
{
    pub base: Base,
    pub staged: Staged,
}

impl<Base: TreeLikeMut, Staged: TreeLikeMut<F = Base::F>> StagedTree<Base, Staged> {
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

impl<Base: TreeLikeMut, Staged: TreeLikeMut<F = Base::F>> TreeLike for StagedTree<Base, Staged> {
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

impl<Base: TreeLikeMut, Staged: TreeLikeMut<F = Base::F>> TreeLikeMut for StagedTree<Base, Staged> {
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
