use crate::crypto::ECSigned;
use crate::file_like::FileLike;
use crate::file_metadata::FileMetadata;
use crate::tree_like::{Stagable, TreeLike};
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

pub type SignedFile = ECSigned<FileMetadata>;

impl Display for SignedFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

impl TreeLike for Option<SignedFile> {
    type F = SignedFile;

    fn ids(&self) -> HashSet<&Uuid> {
        let mut hashset = HashSet::new();
        if let Some(f) = self {
            hashset.insert(f.id());
        }
        hashset
    }

    fn maybe_find(&self, id: &Uuid) -> Option<&SignedFile> {
        if let Some(f) = self {
            if id == f.id() {
                self.as_ref()
            } else {
                None
            }
        } else {
            None
        }
    }

    fn insert(&mut self, f: SignedFile) -> Option<SignedFile> {
        self.replace(f)
    }

    fn remove(&mut self, id: Uuid) -> Option<SignedFile> {
        if let Some(f) = self {
            if &id == f.id() {
                self.take()
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl Stagable for Option<SignedFile> {}

#[cfg(test)]
mod unit_tests {
    use crate::account::Account;
    use crate::file_metadata::FileMetadata;
    use crate::tree_like::TreeLike;
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
        files.find(&file2.id)?;
        files.find(&file3.id)?;

        assert!(files.maybe_find(&Uuid::new_v4()).is_none());

        assert_eq!(files.ids().len(), 3);

        TreeLike::remove(&mut files, file2.id).unwrap();

        assert_eq!(files.ids().len(), 2);

        Ok(())
    }
}
