use crate::account::Account;
use crate::file_like::FileLike;
use crate::file_metadata::{FileMetadata, FileType};
use crate::lazy::LazyTree;
use crate::secret_filename::SecretFileName;
use crate::signed_file::SignedFile;
use crate::staged::StagedTree;
use crate::tree_like::TreeLike;
use crate::SharedResult;
use libsecp256k1::PublicKey;
use uuid::Uuid;

impl<T: TreeLike<SignedFile>> LazyTree<SignedFile, T> {
    fn create(
        &mut self, parent: Uuid, name: &str, file_type: FileType, account: &Account,
        pub_key: &PublicKey,
    ) -> SharedResult<SignedFile> {
        let parent_key = self.decrypt_key(parent, account)?;
        let file =
            FileMetadata::create(pub_key, parent, &parent_key, name, file_type)?.sign(account)?;
        let mut staged = LazyTree::new(StagedTree::new(self, &file));
        staged.validate()?;
        Ok(file)
    }

    fn rename(&mut self, id: Uuid, name: &str, account: &Account) -> SharedResult<SignedFile> {
        let mut file = self.find(id)?.timestamped_value.value.clone();
        let parent_key = self.decrypt_key(file.parent(), account)?;
        file.name = SecretFileName::from_str(name, &parent_key)?;
        let file = file.sign(account)?;
        let mut staged = LazyTree::new(StagedTree::new(self, &file));
        staged.validate()?;
        Ok(file)
    }
}
