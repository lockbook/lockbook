use crate::file::like::FileLike;
use crate::tree::lazy::{LazyTree, LazyTreeRef};
use crate::tree::like::{TreeLike, TreeLikeMut};
use crate::tree::staged::{StagedTree, StagedTreeRef};



pub trait Stagable: TreeLike {
    fn stage<'b, 's, Staged>(&'b self, staged: &'s Staged) -> StagedTreeRef<'b, 's, Self, Staged>
    where
        Staged: StagableMut<F = Self::F>,
        Self: Sized,
    {
        StagedTreeRef::new(self, staged)
    }

    fn as_lazy(&self) -> LazyTreeRef<Self> {
        LazyTreeRef::new(self)
    }
}

impl<T> Stagable for &T where T: Stagable {}
impl<T> Stagable for &mut T where T: Stagable {}

pub trait StagableMut: Stagable + TreeLikeMut {
    fn stage_mut<Staged>(self, staged: Staged) -> StagedTree<Self, Staged>
    where
        Staged: StagableMut<F = Self::F>,
        Self: Sized,
    {
        StagedTree::new(self, staged)
    }

    fn to_lazy(self) -> LazyTree<Self> {
        LazyTree::new(self)
    }
}

impl<T> StagableMut for &mut T where T: StagableMut {}

impl<Base: StagableMut, Staged: StagableMut<F = Base::F>> Stagable for StagedTree<Base, Staged> {}

impl<'s, Base: StagableMut, Staged: StagableMut<F = Base::F>> StagableMut
    for StagedTree<Base, Staged>
{
}

impl<F> Stagable for Option<F> where F: FileLike {}
impl<F> StagableMut for Option<F> where F: FileLike {}

impl<F: FileLike> Stagable for Vec<F> {}
impl<F: FileLike> StagableMut for Vec<F> {}
