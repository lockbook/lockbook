use crate::file_like::FileLike;
use crate::tree_like::{Stagable, TreeLike};
use std::collections::HashSet;
use uuid::Uuid;

pub struct StagedTree<Base, Staged>
where
    Base: Stagable,
    Staged: Stagable<F = Base::F>,
{
    pub base: Base,
    pub staged: Staged,
}

impl<Base: Stagable, Staged: Stagable<F = Base::F>> StagedTree<Base, Staged> {
    pub fn new(base: Base, staged: Staged) -> Self {
        Self { base, staged }
    }
}

impl<Base: Stagable, Staged: Stagable<F = Base::F>> TreeLike for StagedTree<Base, Staged> {
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

impl<Base: Stagable, Staged: Stagable<F = Base::F>> Stagable for StagedTree<Base, Staged> {}

impl<F: FileLike> Stagable for Vec<F> {}
