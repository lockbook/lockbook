use crate::file_like::FileLike;
use crate::tree_like::TreeLike;
use std::collections::HashSet;
use std::fmt;
use std::fmt::Display;
use std::marker::PhantomData;
use uuid::Uuid;

#[derive(Clone)]
pub enum StagedFile<Base: FileLike, Staged: FileLike> {
    Base(Base),
    Staged(Staged),
    Both { base: Base, staged: Staged },
}

impl<'a, Base: FileLike, Staged: FileLike> Display for StagedFile<Base, Staged> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

pub struct StagedTree<'a, F: FileLike, Base: TreeLike<F>, Staged: TreeLike<F>> {
    base: &'a Base,
    staged: &'a Staged,
    _f: PhantomData<F>,
}

impl<'a, F: FileLike, Base: TreeLike<F>, Staged: TreeLike<F>> StagedTree<'a, F, Base, Staged> {
    pub fn new(base: &'a Base, staged: &'a Staged) -> Self {
        Self { base, staged, _f: Default::default() }
    }
}

impl<'a, F: FileLike, Base: TreeLike<F>, Staged: TreeLike<F>> TreeLike<F>
    for StagedTree<'a, F, Base, Staged>
{
    fn ids(&self) -> HashSet<Uuid> {
        self.base
            .ids()
            .into_iter()
            .chain(self.staged.ids().into_iter())
            .collect()
    }

    fn maybe_find(&self, id: Uuid) -> Option<&F> {
        match (self.base.maybe_find(id), self.staged.maybe_find(id)) {
            (_, Some(staged)) => Some(staged),
            (Some(base), None) => Some(base),
            (None, None) => None,
        }
    }
}
