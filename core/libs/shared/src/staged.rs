use crate::file_like::FileLike;
use crate::tree_like::{Stagable, TreeLike};
use std::collections::HashSet;
use std::fmt;
use std::fmt::Display;
use uuid::Uuid;

#[derive(Clone, PartialEq)]
pub enum StagedFile<Base: FileLike, Staged: FileLike> {
    Base(Base),
    Staged(Staged),
    Both { base: Base, staged: Staged },
}

impl<Base: FileLike, Staged: FileLike> Display for StagedFile<Base, Staged> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

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

// pub type NestedStage<'a, F, T1, T2, T3> = StagedTree<'a, F, T1, StagedTree<'a, F, T2, T3>>;
//
// impl<'a, F: FileLike, Base: TreeLike<F>, StagedBase: TreeLike<F>, StagedStaged: TreeLike<F>>
//     NestedStage<'a, F, Base, StagedBase, StagedStaged>
// {
//     fn promote(&mut self) -> SharedResult<()> {
//         todo!()
//     }
// }
