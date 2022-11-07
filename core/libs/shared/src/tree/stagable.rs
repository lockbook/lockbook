use crate::tree::lazy::{LazyTree, LazyTreeRef};
use crate::tree::like::TreeLikeMut;
use crate::tree::staged::StagedTree;

pub trait Stagable: TreeLikeMut {
    fn stage<Staged>(self, staged: &mut Staged) -> StagedTree<Self, Staged>
    where
        Staged: Stagable<F = Self::F>,
        Self: Sized,
    {
        StagedTree::new(self, staged)
    }

    fn to_lazy(self) -> LazyTree<Self> {
        LazyTree::new(self)
    }

    fn as_lazy(&self) -> LazyTreeRef<Self> {
        LazyTreeRef::new(self)
    }
}

impl<T> Stagable for &mut T where T: Stagable {}
