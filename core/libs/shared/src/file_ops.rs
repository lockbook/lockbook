use std::collections::HashSet;

use crate::account::Account;
use crate::file_like::FileLike;
use crate::file_metadata::{FileMetadata, FileType};
use crate::lazy::{LazyStaged1, LazyTree};
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

    /// Removes deleted files which are safe to delete. Call this function after a set of operations rather than in-between
    /// each operation because otherwise you'll prune e.g. a file that was moved out of a folder that was deleted.
    pub fn prunable_ids(&mut self) -> SharedResult<HashSet<Uuid>> {
        // If a file is deleted or has a deleted ancestor, we say that it is deleted. Whether a file is deleted is specific
        // to the source (base or local). We cannot prune (delete from disk) a file in one source and not in the other in
        // order to preserve the semantics of having a file present on one, the other, or both (unmodified/new/modified).
        // For a file to be pruned, it must be deleted on both sources but also have no non-deleted descendants on either
        // source - otherwise, the metadata for those descendants can no longer be decrypted. For an example of a situation
        // where this is important, see the test prune_deleted_document_moved_from_deleted_folder_local_only.

        // find files deleted on base and local; new deleted local files are also eligible
        
        let (base, deleted_base) = {
            let mut tree = LazyTree::new(self.tree.base.all_files()?.into_iter().cloned().collect::<Vec<SignedFile>>());
            let mut deleted = HashSet::new();
            for id in tree.owned_ids() {
                if tree.calculate_deleted(&id)? {
                    deleted.insert(id);
                }
            }
            (tree, deleted)
        };
        let (staged, deleted_staged) = {
            let tree = self;
            let mut deleted = HashSet::new();
            for id in tree.owned_ids() {
                if tree.calculate_deleted(&id)? {
                    deleted.insert(id);
                }
            }
            (tree, deleted)
        };

        let deleted_either = deleted_base.union(&deleted_staged).map(|&id| id).collect();
        let not_deleted_either = staged.owned_ids().difference(&deleted_either).map(|&id| id).collect::<HashSet<_>>();

        // exclude files with not deleted descendants i.e. exclude files that are the ancestors of not deleted files
        let mut to_prune = deleted_either;
        for id in not_deleted_either {
            for ancestor in base.ancestors(&id)? {
                to_prune.remove(&ancestor);
            }
            for ancestor in staged.ancestors(&id)? {
                to_prune.remove(&ancestor);
            }
        }

        Ok(to_prune)
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
