use crate::account::Account;
use crate::crypto::EncryptedDocument;
use crate::file::File;
use crate::file_like::FileLike;
use crate::file_metadata::{FileMetadata, FileType};
use crate::filename::{DocumentType, NameComponents};
use crate::lazy::{LazyStage2, LazyStaged1, LazyTree, Stage1};
use crate::secret_filename::{HmacSha256, SecretFileName};
use crate::signed_file::SignedFile;
use crate::staged::StagedTree;
use crate::tree_like::{Stagable, TreeLike};
use crate::{compression_service, symkey, validate, SharedError, SharedResult};
use hmac::{Mac, NewMac};
use libsecp256k1::PublicKey;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub type TreeWithOp<Base, Local> = LazyTree<StagedTree<Stage1<Base, Local>, Option<SignedFile>>>;
pub type TreeWithOps<Base, Local> = LazyTree<StagedTree<Stage1<Base, Local>, Vec<SignedFile>>>;

impl<Base, Local> LazyStaged1<Base, Local>
where
    Base: Stagable<F = SignedFile>,
    Local: Stagable<F = Base::F>,
{
    pub fn finalize(&mut self, id: &Uuid, account: &Account) -> SharedResult<File> {
        let meta = self.find(id)?;
        let file_type = meta.file_type();
        let parent = *meta.parent();
        let last_modified = meta.timestamped_value.timestamp as u64;
        let name = self.name(id, account)?;
        let id = *id;
        let last_modified_by = account.username.clone();

        Ok(File { id, parent, name, file_type, last_modified, last_modified_by })
    }

    pub fn create(
        self, parent: &Uuid, name: &str, file_type: FileType, account: &Account,
        pub_key: &PublicKey,
    ) -> SharedResult<(Self, Uuid)> {
        let (mut tree, id) = self.stage_create(parent, name, file_type, account, pub_key)?;
        tree.validate()?;
        let tree = tree.promote();
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
        let tree = tree.promote();
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

        Ok(tree.promote())
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
        let tree = tree.promote();
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

    // todo: validate, split out non-validating version (stage_update_document)
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
    // todo: incrementalism
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

    // assumptions: no orphans
    // changes: moves files
    // invalidated by: moved files
    // todo: incrementalism
    pub fn unmove_moved_files_in_cycles(
        self, account: &Account,
    ) -> SharedResult<TreeWithOps<Base, Local>> {
        let mut result = self.stage(Vec::new());

        let mut root_found = false;
        let mut no_cycles_in_ancestors = HashSet::new();
        let mut to_revert = HashSet::new();
        for id in result.owned_ids() {
            let mut ancestors = HashSet::new();
            let mut current_file = result.find(&id)?;
            loop {
                if no_cycles_in_ancestors.contains(current_file.id()) {
                    break;
                } else if current_file.is_root() {
                    if !root_found {
                        root_found = true;
                        ancestors.insert(*current_file.id());
                        break;
                    } else {
                        to_revert.insert(id);
                        break;
                    }
                } else if ancestors.contains(current_file.parent()) {
                    to_revert.extend(result.ancestors(current_file.id())?);
                    break;
                }
                ancestors.insert(*current_file.id());
                current_file = result.find_parent(current_file)?;
            }
            no_cycles_in_ancestors.extend(ancestors);
        }
        for id in to_revert {
            if let (Some(base), Some(_)) =
                (result.tree.base.maybe_find(&id), result.tree.staged.maybe_find(&id))
            {
                let parent = *base.parent();
                result = result.stage_move(&id, &parent, account)?.promote();
            }
        }
        Ok(result)
    }

    // assumptions: no orphans
    // changes: renames files
    // invalidated by: moved files, renamed files
    // todo: incrementalism
    pub fn rename_files_with_path_conflicts(
        self, account: &Account,
    ) -> SharedResult<TreeWithOps<Base, Local>> {
        let mut result = self.stage(Vec::new());

        for (id, sibling_ids) in result.all_children()?.clone() {
            for sibling_id in sibling_ids.iter() {
                let mut name = result.name(&sibling_id, account)?;
                let mut changed = false;
                while sibling_ids
                    .iter()
                    .map(|id| result.name(id, account))
                    .propagate_err()?
                    .any(|sibling_name| sibling_name == name)
                {
                    name = NameComponents::from(&name).generate_next().to_name();
                    changed = true;
                }
                if changed {
                    result = result.stage_rename(&id, &name, account)?.promote();
                }
            }
        }

        Ok(result)
    }
}

impl<Base, Remote, Local> LazyStage2<Base, Remote, Local>
where
    Base: Stagable<F = SignedFile>,
    Remote: Stagable<F = Base::F>,
    Local: Stagable<F = Base::F>,
{
    /// Applies changes to local such that this is a valid tree.
    pub fn merge(
        self, account: &Account, base_documents: &HashMap<Uuid, EncryptedDocument>,
        local_document_changes: &HashMap<Uuid, EncryptedDocument>,
        remote_document_changes: &HashMap<Uuid, EncryptedDocument>,
    ) -> SharedResult<(Self, HashMap<Uuid, EncryptedDocument>)>
    where
        Base: Stagable<F = SignedFile>,
        Remote: Stagable<F = SignedFile>,
        Local: Stagable<F = SignedFile>,
    {
        let mut result = self.stage(Vec::new());
        let mut merge_document_changes = HashMap::new();

        // merge files on an individual basis
        {
            for id in result.tree.base.base.staged.owned_ids() {
                if result.tree.base.staged.maybe_find(&id).is_some() {
                    // 3-way merge
                    if result.tree.base.base.base.maybe_find(&id).is_some() {
                        let (
                            base_file,
                            base_name,
                            remote_change,
                            remote_name,
                            remote_deleted,
                            local_change,
                            local_name,
                        ) = {
                            let (local, merge_changes) = result.unstage();
                            let (remote, local_changes) = local.unstage();
                            let (mut base, remote_changes) = remote.unstage();
                            let base_file = base.find(&id)?.clone();
                            let remote_change = remote_changes.find(&id)?.clone();
                            let local_change = local_changes.find(&id)?.clone();
                            let base_name = base.name(&id, account)?;
                            let mut remote = base.stage(remote_changes);
                            let remote_name = remote.name(&id, account)?;
                            let remote_deleted = remote.calculate_deleted(&id)?;
                            let mut local = remote.stage(local_changes);
                            let local_name = local.name(&id, account)?;
                            result = local.stage(merge_changes);
                            (
                                base_file,
                                base_name,
                                remote_change,
                                remote_name,
                                remote_deleted,
                                local_change,
                                local_name,
                            )
                        };

                        let parent = *three_way_merge(
                            base_file.parent(),
                            remote_change.parent(),
                            local_change.parent(),
                            remote_change.parent(),
                        );

                        let name = SecretFileName::from_str(
                            three_way_merge(&base_name, &remote_name, &local_name, &remote_name),
                            &result.decrypt_key(&id, account)?,
                            &result.decrypt_key(&parent, account)?,
                        )?;
                        result.insert(
                            FileMetadata {
                                id,
                                file_type: base_file.file_type(),
                                parent,
                                name,
                                owner: base_file.owner(),
                                is_deleted: remote_deleted | local_change.explicitly_deleted(),
                                document_hmac: three_way_merge(
                                    &base_file.document_hmac(),
                                    &remote_change.document_hmac(),
                                    &local_change.document_hmac(),
                                    &None, // overwritten during document merge if local != remote
                                )
                                .cloned(),
                                user_access_keys: base_file.user_access_keys().clone(),
                                folder_access_keys: base_file.folder_access_keys().clone(),
                            }
                            .sign(account)?,
                        );
                    }
                    // 2-way merge
                    else {
                        let (remote_change, remote_name, remote_deleted, local_change) = {
                            let (local, merge_changes) = result.unstage();
                            let (remote, local_changes) = local.unstage();
                            let (base, remote_changes) = remote.unstage();
                            let remote_change = remote_changes.find(&id)?.clone();
                            let local_change = local_changes.find(&id)?.clone();
                            let mut remote = base.stage(remote_changes);
                            let remote_name = remote.name(&id, account)?;
                            let remote_deleted = remote.calculate_deleted(&id)?;
                            let local = remote.stage(local_changes);
                            result = local.stage(merge_changes);
                            (remote_change, remote_name, remote_deleted, local_change)
                        };

                        let key = result.decrypt_key(&id, account)?;
                        let parent_key = result.decrypt_key(remote_change.parent(), account)?;
                        result.insert(
                            FileMetadata {
                                id,
                                file_type: remote_change.file_type(),
                                parent: *remote_change.parent(),
                                name: SecretFileName::from_str(&remote_name, &key, &parent_key)?,
                                owner: remote_change.owner(),
                                is_deleted: remote_deleted | local_change.explicitly_deleted(),
                                document_hmac: remote_change.document_hmac().cloned(), // overwritten during document merge if local != remote
                                user_access_keys: remote_change.user_access_keys().clone(),
                                folder_access_keys: remote_change.folder_access_keys().clone(),
                            }
                            .sign(account)?,
                        );
                    }
                }
            }
        }

        // merge documents
        {
            for (id, remote_document_change) in remote_document_changes {
                // todo: use merged document type
                let local_document_type =
                    DocumentType::from_file_name_using_extension(&result.name(id, account)?);
                result = match (local_document_changes.get(id), local_document_type) {
                    // no local changes -> no merge
                    (None, _) => result,
                    // text files always merged
                    (Some(local_document_change), DocumentType::Text) => {
                        let (
                            decrypted_base_document,
                            decrypted_remote_document,
                            decrypted_local_document,
                        ) = {
                            let (local, merge_changes) = result.unstage();
                            let (remote, local_changes) = local.unstage();
                            let (mut base, remote_changes) = remote.unstage();
                            let decrypted_base_document = base_documents
                                .get(id)
                                .map(|document| base.decrypt_document(id, document, account))
                                .map_or(Ok(None), |v| v.map(Some))?
                                .unwrap_or_default();
                            let mut remote = base.stage(remote_changes);
                            let decrypted_remote_document =
                                remote.decrypt_document(id, remote_document_change, account)?;
                            let mut local = remote.stage(local_changes);
                            let decrypted_local_document =
                                local.decrypt_document(id, local_document_change, account)?;
                            result = local.stage(merge_changes);
                            (
                                decrypted_base_document,
                                decrypted_remote_document,
                                decrypted_local_document,
                            )
                        };

                        let merged_document = match diffy::merge_bytes(
                            &decrypted_base_document,
                            &decrypted_remote_document,
                            &decrypted_local_document,
                        ) {
                            Ok(without_conflicts) => without_conflicts,
                            Err(with_conflicts) => with_conflicts,
                        };
                        let (result, encrypted_document) =
                            result.update_document(id, &merged_document, account)?;
                        merge_document_changes.insert(*id, encrypted_document);
                        result
                    }
                    // non-text files always duplicated
                    (Some(local_document_change), DocumentType::Drawing | DocumentType::Other) => {
                        let (decrypted_remote_document, decrypted_local_document) = {
                            let (local, merge_changes) = result.unstage();
                            let (mut remote, local_changes) = local.unstage();
                            let decrypted_remote_document =
                                remote.decrypt_document(id, remote_document_change, account)?;
                            let mut local = remote.stage(local_changes);
                            let decrypted_local_document =
                                local.decrypt_document(id, local_document_change, account)?;
                            result = local.stage(merge_changes);
                            (decrypted_remote_document, decrypted_local_document)
                        };

                        // overwrite existing document (todo: avoid decrypting and re-encrypting document)
                        let (mut result, encrypted_document) =
                            result.update_document(id, &decrypted_remote_document, account)?;
                        merge_document_changes.insert(*id, encrypted_document);

                        // create copied document (todo: avoid decrypting and re-encrypting document)
                        let (&existing_parent, existing_file_type) = {
                            let existing_document = result.find(id)?;
                            (existing_document.parent(), existing_document.file_type())
                        };

                        let name = result.name(id, account)?;
                        let (result, copied_document_id) = result.stage_create(
                            &existing_parent,
                            &name,
                            existing_file_type,
                            account,
                            &account.public_key(),
                        )?;
                        let result = result.promote();
                        let (result, encrypted_document) = result.update_document(
                            &copied_document_id,
                            &decrypted_local_document,
                            account,
                        )?;
                        merge_document_changes.insert(*id, encrypted_document);

                        result
                    }
                }
            }
        }

        // resolve tree merge conflicts
        result = result.unmove_moved_files_in_cycles(account)?.promote();
        result = result.rename_files_with_path_conflicts(account)?.promote();

        // todo: validate?

        Ok((result.promote(), merge_document_changes))
    }
}

/// Returns the 3-way merge of any comparable value; returns `resolution` in the event of a conflict.
///
/// # Examples
///
/// ```
/// let (base, local, remote) = ("hello", "hello local", "hello remote");
/// let result = merge(base, local, remote, remote);
/// assert_eq!(result, "hello remote");
/// ```
pub fn three_way_merge<'a, T: Eq + ?Sized>(
    base: &'a T, remote: &'a T, local: &'a T, resolution: &'a T,
) -> &'a T {
    let remote_changed = remote != base;
    let local_changed = local != base;
    match (remote_changed, local_changed) {
        (false, false) => base,
        (false, true) => local,
        (true, false) => remote,
        (true, true) => resolution,
    }
}

trait ResultIterator<Item, Error> {
    fn propagate_err(self) -> Result<std::vec::IntoIter<Item>, Error>
    where
        Self: Sized;
}

impl<I, Item, Error> ResultIterator<Item, Error> for I
where
    I: Iterator<Item = Result<Item, Error>>,
{
    fn propagate_err(self) -> Result<std::vec::IntoIter<Item>, Error>
    where
        Self: Sized,
    {
        Ok(self.collect::<Result<Vec<Item>, Error>>()?.into_iter())
    }
}
