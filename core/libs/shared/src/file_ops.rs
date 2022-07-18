use crate::account::Account;
use crate::crypto::{DecryptedDocument, EncryptedDocument};
use crate::file::File;
use crate::file_like::FileLike;
use crate::file_metadata::{FileMetadata, FileType};
use crate::lazy::LazyTree;
use crate::secret_filename::SecretFileName;
use crate::signed_file::SignedFile;
use crate::staged::StagedTree;
use crate::tree_like::TreeLike;
use crate::{symkey, SharedResult};
use libsecp256k1::PublicKey;
use uuid::Uuid;

impl<T: TreeLike<SignedFile>> LazyTree<SignedFile, T> {
    pub fn create(
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

    pub fn rename(&mut self, id: Uuid, name: &str, account: &Account) -> SharedResult<SignedFile> {
        let mut file = self.find(id)?.timestamped_value.value.clone();
        let parent_key = self.decrypt_key(file.parent(), account)?;
        file.name = SecretFileName::from_str(name, &parent_key)?;
        let file = file.sign(account)?;
        let mut staged = LazyTree::new(StagedTree::new(self, &file));
        staged.validate()?;
        Ok(file)
    }

    pub fn encrypt_document(
        &mut self, id: Uuid, document: &DecryptedDocument, account: &Account,
    ) -> SharedResult<EncryptedDocument> {
        let key = self.decrypt_key(id, account)?;
        symkey::encrypt(&key, document)
    }

    pub fn decrypt_document(
        &mut self, id: Uuid, encrypted: &EncryptedDocument, account: &Account,
    ) -> SharedResult<DecryptedDocument> {
        let key = self.decrypt_key(id, account)?;
        symkey::decrypt(&key, encrypted)
    }

    pub fn finalize(&mut self, id: Uuid, account: &Account) -> SharedResult<File> {
        let meta = self.find(id)?;
        let file_type = meta.file_type();
        let parent = meta.parent();
        let name = self.name(id, account)?;
        Ok(File { id, parent, name, file_type })
    }
}
