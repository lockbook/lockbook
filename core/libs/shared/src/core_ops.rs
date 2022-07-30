use std::collections::HashSet;

use crate::account::Account;
use crate::crypto::{DecryptedDocument, EncryptedDocument};
use crate::file_like::FileLike;
use crate::file_metadata::{FileMetadata, FileType};
use crate::lazy::{LazyStaged1, LazyTree, Stage1};
use crate::secret_filename::{HmacSha256, SecretFileName};
use crate::signed_file::SignedFile;
use crate::staged::StagedTree;
use crate::tree_like::{Stagable, TreeLike};
use crate::{compression_service, symkey, validate, SharedError, SharedResult};
use hmac::{Mac, NewMac};
use libsecp256k1::PublicKey;
use uuid::Uuid;

pub type TreeWithOp<Base, Local> = LazyTree<StagedTree<Stage1<Base, Local>, Option<SignedFile>>>;

impl<Base, Local> LazyStaged1<Base, Local>
where
    Base: Stagable<F = SignedFile>,
    Local: Stagable<F = Base::F>,
{
    pub fn create(
        self, parent: &Uuid, name: &str, file_type: FileType, account: &Account,
        pub_key: &PublicKey,
    ) -> SharedResult<(Self, Uuid)> {
        let (mut tree, id) = self.stage_create(parent, name, file_type, account, pub_key)?;
        tree.validate()?;
        let tree = tree.promote_to_local();
        Ok((tree, id))
    }

    pub fn stage_create(
        mut self, parent: &Uuid, name: &str, file_type: FileType, account: &Account,
        pub_key: &PublicKey,
    ) -> SharedResult<(TreeWithOp<Base, Local>, Uuid)> {
        validate::file_name(name)?;

        if self.calculate_deleted(parent)? {
            return Err(SharedError::FileParentNonexistent);
        }

        let parent_key = self.decrypt_key(parent, account)?;
        let new_file =
            FileMetadata::create(pub_key, *parent, &parent_key, name, file_type)?.sign(account)?;
        let id = *new_file.id();
        Ok((self.stage(Some(new_file)), id))
    }

    pub fn rename(self, id: &Uuid, name: &str, account: &Account) -> SharedResult<Self> {
        let mut tree = self.stage_rename(id, name, account)?;
        tree.validate()?;
        let tree = tree.promote_to_local();
        Ok(tree)
    }

    pub fn stage_rename(
        mut self, id: &Uuid, name: &str, account: &Account,
    ) -> SharedResult<TreeWithOp<Base, Local>> {
        let mut file = self.find(id)?.timestamped_value.value.clone();

        validate::file_name(name)?;
        validate::not_root(&file)?;

        if self.calculate_deleted(id)? {
            return Err(SharedError::FileNonexistent);
        }

        let parent_key = self.decrypt_key(file.parent(), account)?;
        let key = self.decrypt_key(id, account)?;
        file.name = SecretFileName::from_str(name, &key, &parent_key)?;
        let file = file.sign(account)?;
        Ok(self.stage(Some(file)))
    }

    pub fn move_file(self, id: &Uuid, new_parent: &Uuid, account: &Account) -> SharedResult<Self> {
        let mut tree = self.stage_move(id, new_parent, account)?;
        tree.validate()?;

        Ok(tree.promote_to_local())
    }

    pub fn stage_move(
        mut self, id: &Uuid, new_parent: &Uuid, account: &Account,
    ) -> SharedResult<TreeWithOp<Base, Local>> {
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

        Ok(self.stage(Some(file)))
    }

    pub fn delete(self, id: &Uuid, account: &Account) -> SharedResult<LazyStaged1<Base, Local>> {
        let mut tree = self.stage_delete(id, account)?;
        tree.validate()?;
        let tree = tree.promote_to_local();
        Ok(tree)
    }

    pub fn stage_delete(
        self, id: &Uuid, account: &Account,
    ) -> SharedResult<TreeWithOp<Base, Local>> {
        let mut file = self.find(id)?.timestamped_value.value.clone();
        validate::not_root(&file)?;
        file.is_deleted = true;
        let file = file.sign(account)?;
        Ok(self.stage(Some(file)))
    }

    pub fn update_document(
        mut self, id: &Uuid, document: &[u8], account: &Account,
    ) -> SharedResult<(Self, EncryptedDocument)> {
        let mut file: FileMetadata = self.find(id)?.timestamped_value.value.clone();
        validate::not_root(&file)?;
        validate::is_document(&file)?;

        let key = self.decrypt_key(id, account)?;
        let hmac = {
            let mut mac =
                HmacSha256::new_from_slice(&key).map_err(SharedError::HmacCreationError)?;
            mac.update(document);
            mac.finalize().into_bytes()
        }
        .into();

        file.document_hmac = Some(hmac);
        let file = file.sign(account)?;

        let document = compression_service::compress(document)?;
        let document = symkey::encrypt(&key, &document)?;

        Ok((self.stage(Some(file)).promote(), document))
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
            let mut tree = LazyTree::new(
                self.tree
                    .base
                    .all_files()?
                    .into_iter()
                    .cloned()
                    .collect::<Vec<SignedFile>>(),
            );
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

        let deleted_both = deleted_base
            .intersection(&deleted_staged)
            .copied()
            .collect();
        let not_deleted_either = staged
            .owned_ids()
            .difference(&deleted_both)
            .copied()
            .collect::<HashSet<_>>();

        // exclude files with not deleted descendants i.e. exclude files that are the ancestors of not deleted files
        let mut to_prune = deleted_both;
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
}
