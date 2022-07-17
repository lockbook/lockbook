use crate::account::Account;
use crate::file_like::FileLike;
use crate::lazy::LazyTree;
use crate::secret_filename::SecretFileName;
use crate::signed_file::SignedFile;
use crate::staged::StagedTree;
use crate::tree_like::{TreeError, TreeLike};
use uuid::Uuid;

impl<T: TreeLike<SignedFile>> LazyTree<SignedFile, T> {
    fn rename(&mut self, id: Uuid, name: &str, account: &Account) -> Result<SignedFile, TreeError> {
        let mut file = self.find(id)?.timestamped_value.value.clone();
        let parent_key = self.decrypt_key(file.parent(), account)?;
        file.name = SecretFileName::from_str(name, &parent_key).unwrap();
        let file = file.sign(account).unwrap();
        let mut staged = LazyTree::new(StagedTree::new(self, &file));
        staged.validate().unwrap();
        Ok(file)
    }
}
