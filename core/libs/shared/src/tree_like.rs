use crate::file_like::FileLike;
use crate::lazy::LazyTree;
use crate::staged::StagedTree;
use crate::{SharedError, SharedResult};
use std::collections::HashSet;
use uuid::Uuid;

pub trait TreeLike: Sized {
    type F: FileLike;

    // TODO perf, it would be nice to make this a reference type, some point in the future
    fn ids(&self) -> HashSet<&Uuid>;
    fn maybe_find(&self, id: &Uuid) -> Option<&Self::F>;
    fn insert(&mut self, f: Self::F) -> Option<Self::F>;
    fn remove(&mut self, id: Uuid) -> Option<Self::F>;

    fn find(&self, id: &Uuid) -> SharedResult<&Self::F> {
        self.maybe_find(id).ok_or(SharedError::FileNonexistent)
    }

    fn maybe_find_parent<F2: FileLike>(&self, file: &F2) -> Option<&Self::F> {
        self.maybe_find(file.parent())
    }

    fn find_parent<F2: FileLike>(&self, file: &F2) -> SharedResult<&Self::F> {
        self.maybe_find_parent(file)
            .ok_or(SharedError::FileParentNonexistent)
    }

    fn owned_ids(&self) -> HashSet<Uuid> {
        self.ids().iter().map(|id| **id).collect()
    }

    fn all_files(&self) -> SharedResult<Vec<&Self::F>> {
        let mut all = vec![];
        for id in self.ids() {
            let meta = self.find(id)?;
            all.push(meta);
        }

        Ok(all)
    }
}

pub trait Stagable: TreeLike {
    fn stage<Staged>(self, staged: Staged) -> StagedTree<Self, Staged>
    where
        Staged: Stagable<F = Self::F>,
        Self: Sized,
    {
        StagedTree::new(self, staged)
    }

    fn to_lazy(self) -> LazyTree<Self> {
        LazyTree::new(self)
    }
}

impl<F> TreeLike for Vec<F>
where
    F: FileLike,
{
    type F = F;

    fn ids(&self) -> HashSet<&Uuid> {
        self.iter().map(|f| f.id()).collect()
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&F> {
        self.iter().find(|f| f.id() == id)
    }

    fn insert(&mut self, f: F) -> Option<F> {
        for (i, value) in self.iter().enumerate() {
            if value.id() == f.id() {
                let old = self.remove(i);
                self[i] = f;
                return Some(old);
            }
        }

        self.push(f);

        None
    }

    fn remove(&mut self, id: Uuid) -> Option<F> {
        for (i, value) in self.iter().enumerate() {
            if *value.id() == id {
                return Some(self.remove(i));
            }
        }

        None
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::account::Account;
    use crate::file_like::FileLike;
    use crate::file_metadata::FileMetadata;
    use crate::tree_like::{Stagable, TreeLike};
    use crate::SharedResult;
    use test_utils::*;
    use uuid::Uuid;

    #[test]
    fn tree_test() -> SharedResult<()> {
        let account = &Account::new(random_name(), url());
        let file1 = FileMetadata::create_root(account)?;
        let file2 = FileMetadata::create_root(account)?;
        let file3 = FileMetadata::create_root(account)?;

        let mut files = vec![file1.clone(), file2.clone(), file3.clone()];

        files.find(&file1.id)?;
        files.find(&file3.id)?;
        files.find(&file3.id)?;

        assert!(files.maybe_find(&Uuid::new_v4()).is_none());

        assert_eq!(files.ids().len(), 3);

        TreeLike::remove(&mut files, file2.id).unwrap();

        assert_eq!(files.ids().len(), 2);
        files.find(&file2.id).unwrap_err();
        assert!(files.maybe_find(&file2.id).is_none());

        Ok(())
    }

    #[test]
    fn test_stage_insert_reset() -> SharedResult<()> {
        let account = &Account::new(random_name(), url());
        let file1 = FileMetadata::create_root(account)?;
        let mut file2 = FileMetadata::create_root(account)?;
        let file3 = FileMetadata::create_root(account)?;

        let mut files = vec![file1.clone(), file2.clone(), file3.clone()];

        let id = Uuid::new_v4();
        file2.parent = id;
        let mut files = files.stage(Some(file2.clone()));

        assert_eq!(files.find(file2.id())?.parent(), &id);
        assert_eq!(files.base.find(file2.id())?.parent(), file2.id());
        assert_eq!(files.ids().len(), 3);

        // Now reset the file

        file2.parent = file2.id;
        files.insert(file2.clone());
        assert_eq!(files.find(file2.id())?.parent(), file2.id());
        assert_eq!(files.base.find(file2.id())?.parent(), file2.id());
        assert!(files.staged.maybe_find(file2.id()).is_none());
        assert_eq!(files.ids().len(), 3);

        Ok(())
    }

    #[test]
    fn test_stage_reset() -> SharedResult<()> {
        let account = &Account::new(random_name(), url());
        let file1 = FileMetadata::create_root(account)?;
        let file2 = FileMetadata::create_root(account)?;
        let file3 = FileMetadata::create_root(account)?;

        let mut files = vec![file1.clone(), file2.clone(), file3.clone()];

        let mut files = files.stage(Some(file2.clone()));

        assert_eq!(files.find(file2.id())?.parent(), file2.id());
        assert_eq!(files.base.find(file2.id())?.parent(), file2.id());
        assert!(files.staged.maybe_find(file2.id()).is_none());

        assert_eq!(files.ids().len(), 3);

        Ok(())
    }
}
