use crate::account::Account;
use crate::file_like::FileLike;
use crate::file_metadata::{FileMetadata, FileType};
use crate::lazy::LazyStaged1;
use crate::secret_filename::SecretFileName;
use crate::signed_file::SignedFile;
use crate::tree_like::{Stagable, TreeLike};
use crate::{symkey, validate, SharedError, SharedResult};
use libsecp256k1::PublicKey;
use uuid::Uuid;

impl<Base, Local> LazyStaged1<Base, Local>
where
    Base: Stagable<F = SignedFile>,
    Local: Stagable<F = Base::F>,
{
    pub fn create(
        mut self, parent: &Uuid, name: &str, file_type: FileType, account: &Account,
        pub_key: &PublicKey,
    ) -> SharedResult<(LazyStaged1<Base, Local>, Uuid)> {
        validate::file_name(name)?;

        if self.calculate_deleted(parent)? {
            return Err(SharedError::FileParentNonexistent);
        }

        let parent_key = self.decrypt_key(parent, account)?;
        let new_file =
            FileMetadata::create(pub_key, *parent, &parent_key, name, file_type)?.sign(account)?;
        let id = *new_file.id();
        let mut staged = self.stage(new_file);
        staged.validate()?;
        Ok((staged.promote(), id))
    }

    pub fn rename(
        mut self, id: &Uuid, name: &str, account: &Account,
    ) -> SharedResult<LazyStaged1<Base, Local>> {
        let mut file = self.find(id)?.timestamped_value.value.clone();

        validate::file_name(name)?;
        validate::not_root(&file)?;

        if self.calculate_deleted(id)? {
            return Err(SharedError::FileNonexistent);
        }

        let parent_key = self.decrypt_key(file.parent(), account)?;
        file.name = SecretFileName::from_str(name, &parent_key)?;
        let file = file.sign(account)?;
        let mut staged = self.stage(file);
        staged.validate()?;
        let tree = staged.promote();
        Ok(tree)
    }

    pub fn move_file(
        mut self, id: &Uuid, new_parent: &Uuid, account: &Account,
    ) -> SharedResult<LazyStaged1<Base, Local>> {
        let mut file = self.find(id)?.timestamped_value.value.clone();
        let parent = self.find(new_parent)?;

        validate::not_root(&file)?;
        validate::is_folder(parent)?;

        if self.calculate_deleted(id)? {
            return Err(SharedError::FileNonexistent);
        }

        if self.calculate_deleted(new_parent)? {
            return Err(SharedError::FileParentNonexistent);
        }

        let key = self.decrypt_key(id, account)?;
        let parent_key = self.decrypt_key(new_parent, account)?;
        file.parent = *new_parent;
        file.folder_access_keys = symkey::encrypt(&parent_key, &key)?;
        let file = file.sign(account)?;

        let mut tree = self.stage(file);
        tree.validate()?;

        Ok(tree.promote())
    }

    pub fn delete(self, id: &Uuid, account: &Account) -> SharedResult<LazyStaged1<Base, Local>> {
        let mut file = self.find(id)?.timestamped_value.value.clone();
        validate::not_root(&file)?;
        file.is_deleted = true;
        let file = file.sign(account)?;
        let mut tree = self.stage(file);
        tree.validate()?;
        let tree = tree.promote();
        Ok(tree)
    }

    pub fn create_at_path(
        mut self, path: &str, root: Uuid, account: &Account, pub_key: &PublicKey,
    ) -> SharedResult<(LazyStaged1<Base, Local>, Uuid)> {
        validate::path(path)?;
        let is_folder = path.ends_with('/');

        let path_components = split_path(path);
        let mut current = root;
        'path: for index in 0..path_components.len() {
            'child: for child in self.children(&current)? {
                if self.calculate_deleted(&child)? {
                    continue 'child;
                }

                if self.name(&child, account)? == path_components[index] {
                    if index == path_components.len() - 1 {
                        return Err(SharedError::PathTaken);
                    }

                    if self.find(&child)?.is_folder() {
                        current = child;
                        continue 'path;
                    } else {
                        return Err(SharedError::FileNotFolder);
                    }
                }
            }

            // Child does not exist, create it
            let file_type = if is_folder || index != path_components.len() - 1 {
                FileType::Folder
            } else {
                FileType::Document
            };

            (self, current) =
                self.create(&current, path_components[index], file_type, account, pub_key)?;
        }

        Ok((self, current))
    }
}

fn split_path(path: &str) -> Vec<&str> {
    path.split('/')
        .collect::<Vec<&str>>()
        .into_iter()
        .filter(|s| !s.is_empty()) // Remove the trailing empty element in the case this is a folder
        .collect::<Vec<&str>>()
}
