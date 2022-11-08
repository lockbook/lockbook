use std::collections::{HashMap, HashSet};

use hmac::{Mac, NewMac};
use hmdb::log::SchemaEvent;
use hmdb::transaction::TransactionTable;
use libsecp256k1::PublicKey;
use tracing::debug;
use uuid::Uuid;

use crate::access_info::{UserAccessInfo, UserAccessMode};
use crate::account::Account;
use crate::core_config::Config;
use crate::crypto::{DecryptedDocument, EncryptedDocument};
use crate::file::{File, Share, ShareMode};
use crate::file_like::FileLike;
use crate::file_metadata::{FileMetadata, FileType, Owner};
use crate::filename::{DocumentType, NameComponents};
use crate::lazy::{LazyStage2, LazyStaged1, LazyTree, LazyTreeLike, Stage1};
use crate::secret_filename::{HmacSha256, SecretFileName};
use crate::signed_file::SignedFile;
use crate::staged::StagedTree;
use crate::tree_like::{TreeLike, TreeLikeMut};
use crate::{compression_service, document_repo, symkey, validate, SharedError, SharedResult};

pub type TreeWithOp<Base, Local> = LazyTree<StagedTree<Stage1<Base, Local>, Option<SignedFile>>>;
pub type TreeWithOps<Base, Local> = LazyTree<StagedTree<Stage1<Base, Local>, Vec<SignedFile>>>;

impl<Base, Local> LazyStaged1<Base, Local>
where
    Base: TreeLikeMut<F = SignedFile>,
    Local: TreeLikeMut<F = Base::F>,
{
    pub fn finalize<PublicKeyCache: SchemaEvent<Owner, String>>(
        &mut self, id: &Uuid, account: &Account,
        public_key_cache: &mut TransactionTable<Owner, String, PublicKeyCache>,
    ) -> SharedResult<File> {
        let meta = self.find(id)?.clone();
        let file_type = meta.file_type();
        let parent = *meta.parent();
        let last_modified = meta.timestamped_value.timestamp as u64;
        let name = self.name(id, account)?;
        let id = *id;
        let last_modified_by = account.username.clone();
        let mut shares = Vec::new();
        for user_access_key in meta.user_access_keys() {
            if user_access_key.encrypted_by == user_access_key.encrypted_for {
                continue;
            }
            let mode = match user_access_key.mode {
                UserAccessMode::Read => ShareMode::Read,
                UserAccessMode::Write => ShareMode::Write,
                UserAccessMode::Owner => continue,
            };
            shares.push(Share {
                mode,
                shared_by: if user_access_key.encrypted_by == account.public_key() {
                    account.username.clone()
                } else {
                    public_key_cache
                        .get(&Owner(user_access_key.encrypted_by))
                        .cloned()
                        .unwrap_or_else(|| String::from("<unknown>"))
                },
                shared_with: if user_access_key.encrypted_for == account.public_key() {
                    account.username.clone()
                } else {
                    public_key_cache
                        .get(&Owner(user_access_key.encrypted_for))
                        .cloned()
                        .unwrap_or_else(|| String::from("<unknown>"))
                },
            });
        }
        Ok(File { id, parent, name, file_type, last_modified, last_modified_by, shares })
    }

    pub fn resolve_and_finalize<I, PublicKeyCache>(
        &mut self, account: &Account, ids: I,
        public_key_cache: &mut TransactionTable<Owner, String, PublicKeyCache>,
    ) -> SharedResult<Vec<File>>
    where
        I: Iterator<Item = Uuid>,
        PublicKeyCache: SchemaEvent<Owner, String>,
    {
        let mut files = Vec::new();
        let mut parent_substitutions = HashMap::new();
        for id in ids {
            if self.calculate_deleted(&id)? {
                continue;
            }
            if self.in_pending_share(&id)? {
                continue;
            }
            if self.link(&id)?.is_some() {
                continue;
            }
            let finalized = self.finalize(&id, account, public_key_cache)?;
            match finalized.file_type {
                FileType::Document | FileType::Folder => files.push(finalized),
                FileType::Link { target } => {
                    let mut target_file = self.finalize(&target, account, public_key_cache)?;
                    if target_file.is_folder() {
                        parent_substitutions.insert(target, id);
                    }
                    target_file.id = finalized.id;
                    target_file.parent = finalized.parent;
                    target_file.name = finalized.name;
                    files.push(target_file);
                }
            }
        }
        for item in &mut files {
            if let Some(new_parent) = parent_substitutions.get(&item.parent) {
                item.parent = *new_parent;
            }
        }
        Ok(files)
    }

    fn create_op(
        &mut self, parent: &Uuid, name: &str, file_type: FileType, account: &Account,
    ) -> SharedResult<(Option<SignedFile>, Uuid)> {
        validate::file_name(name)?;

        if self.calculate_deleted(parent)? {
            return Err(SharedError::FileParentNonexistent);
        }

        let parent_owner = self.find(parent)?.owner().0;
        let parent_key = self.decrypt_key(parent, account)?;
        let file = FileMetadata::create(&parent_owner, *parent, &parent_key, name, file_type)?
            .sign(account)?;
        let id = *file.id();

        debug!("new {:?} with id: {}", file_type, id);
        Ok((Some(file), id))
    }

    pub fn create_unvalidated(
        &mut self, parent: &Uuid, name: &str, file_type: FileType, account: &Account,
    ) -> SharedResult<Uuid> {
        let (op, id) = self.create_op(parent, name, file_type, account)?;
        self.stage_and_promote(op);
        Ok(id)
    }

    pub fn create(
        &mut self, parent: &Uuid, name: &str, file_type: FileType, account: &Account,
    ) -> SharedResult<Uuid> {
        let (op, id) = self.create_op(parent, name, file_type, account)?;
        self.stage_validate_and_promote(op, Owner(account.public_key()))?;
        Ok(id)
    }

    fn rename_op(
        &mut self, id: &Uuid, name: &str, account: &Account,
    ) -> SharedResult<Option<SignedFile>> {
        let mut file = self.find(id)?.timestamped_value.value.clone();

        validate::file_name(name)?;
        if self.maybe_find(file.parent()).is_none() {
            return Err(SharedError::NotPermissioned);
        }
        let parent_key = self.decrypt_key(file.parent(), account)?;
        let key = self.decrypt_key(id, account)?;
        file.name = SecretFileName::from_str(name, &key, &parent_key)?;
        let file = file.sign(account)?;

        Ok(Some(file))
    }

    pub fn rename_unvalidated(
        &mut self, id: &Uuid, name: &str, account: &Account,
    ) -> SharedResult<()> {
        let op = self.rename_op(id, name, account)?;
        self.stage_and_promote(op);
        Ok(())
    }

    pub fn rename(&mut self, id: &Uuid, name: &str, account: &Account) -> SharedResult<()> {
        let op = self.rename_op(id, name, account)?;
        self.stage_validate_and_promote(op, Owner(account.public_key()))?;
        Ok(())
    }

    fn move_op(
        &mut self, id: &Uuid, new_parent: &Uuid, account: &Account,
    ) -> SharedResult<Vec<SignedFile>> {
        let mut file = self.find(id)?.timestamped_value.value.clone();
        if self.maybe_find(new_parent).is_none() || self.calculate_deleted(new_parent)? {
            return Err(SharedError::FileParentNonexistent);
        }
        let key = self.decrypt_key(id, account)?;
        let parent_key = self.decrypt_key(new_parent, account)?;
        let owner = self.find(new_parent)?.owner();
        file.owner = owner;
        file.parent = *new_parent;
        file.folder_access_key = symkey::encrypt(&parent_key, &key)?;
        file.name = SecretFileName::from_str(&self.name(id, account)?, &key, &parent_key)?;
        let file = file.sign(account)?;

        let mut result = vec![file];
        for id in self.descendants(id)? {
            let mut descendant = self.find(&id)?.timestamped_value.value.clone();
            descendant.owner = owner;
            result.push(descendant.sign(account)?);
        }

        Ok(result)
    }

    pub fn move_unvalidated(
        &mut self, id: &Uuid, new_parent: &Uuid, account: &Account,
    ) -> SharedResult<()> {
        let op = self.move_op(id, new_parent, account)?;
        self.stage_and_promote(op);
        Ok(())
    }

    pub fn move_file(
        &mut self, id: &Uuid, new_parent: &Uuid, account: &Account,
    ) -> SharedResult<()> {
        let op = self.move_op(id, new_parent, account)?;
        self.stage_validate_and_promote(op, Owner(account.public_key()))?;
        Ok(())
    }

    fn delete_op(&self, id: &Uuid, account: &Account) -> SharedResult<Option<SignedFile>> {
        let mut file = self.find(id)?.timestamped_value.value.clone();

        file.is_deleted = true;
        let file = file.sign(account)?;

        Ok(Some(file))
    }

    pub fn delete_unvalidated(&mut self, id: &Uuid, account: &Account) -> SharedResult<()> {
        let op = self.delete_op(id, account)?;
        self.stage_and_promote(op);
        Ok(())
    }

    pub fn delete(&mut self, id: &Uuid, account: &Account) -> SharedResult<()> {
        let op = self.delete_op(id, account)?;
        self.stage_validate_and_promote(op, Owner(account.public_key()))?;
        Ok(())
    }

    fn delete_share_op(
        &mut self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>, account: &Account,
    ) -> SharedResult<Vec<SignedFile>> {
        let mut result = Vec::new();
        let mut file = self.find(id)?.timestamped_value.value.clone();

        let mut found = false;
        for key in file.user_access_keys.iter_mut() {
            if let Some(encrypted_for) = maybe_encrypted_for {
                if !key.deleted && key.encrypted_for == encrypted_for {
                    found = true;
                    key.deleted = true;
                }
            } else if !key.deleted {
                found = true;
                key.deleted = true;
            }
        }
        if !found {
            return Err(SharedError::ShareNonexistent);
        }
        result.push(file.sign(account)?);

        // delete any links pointing to file
        if let Some(encrypted_for) = maybe_encrypted_for {
            if encrypted_for == account.public_key() {
                if let Some(link) = self.link(id)? {
                    let mut link = self.find(&link)?.timestamped_value.value.clone();
                    link.is_deleted = true;
                    result.push(link.sign(account)?);
                }
            }
        }

        Ok(result)
    }

    pub fn delete_share_unvalidated(
        &mut self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>, account: &Account,
    ) -> SharedResult<()> {
        let op = self.delete_share_op(id, maybe_encrypted_for, account)?;
        self.stage_and_promote(op);
        Ok(())
    }

    pub fn delete_share(
        &mut self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>, account: &Account,
    ) -> SharedResult<()> {
        let op = self.delete_share_op(id, maybe_encrypted_for, account)?;
        self.stage_validate_and_promote(op, Owner(account.public_key()))?;
        Ok(())
    }

    pub fn read_document(
        &mut self, config: &Config, id: &Uuid, account: &Account,
    ) -> SharedResult<DecryptedDocument> {
        if self.calculate_deleted(id)? {
            return Err(SharedError::FileNonexistent);
        }
        let (id, meta) = if let FileType::Link { target } = self.find(id)?.file_type() {
            (target, self.find(&target)?)
        } else {
            (*id, self.find(id)?)
        };

        validate::is_document(meta)?;
        if meta.document_hmac().is_none() {
            return Ok(vec![]);
        }

        let maybe_encrypted_document =
            match document_repo::maybe_get(config, meta.id(), meta.document_hmac())? {
                Some(local) => Some(local),
                None => document_repo::maybe_get(config, meta.id(), meta.document_hmac())?,
            };
        let doc = match maybe_encrypted_document {
            Some(doc) => self.decrypt_document(&id, &doc, account)?,
            None => return Err(SharedError::FileNonexistent),
        };

        Ok(doc)
    }

    fn update_document_op(
        &mut self, id: &Uuid, document: &[u8], account: &Account,
    ) -> SharedResult<(Option<SignedFile>, EncryptedDocument)> {
        let id = match self.find(id)?.file_type() {
            FileType::Document | FileType::Folder => *id,
            FileType::Link { target } => target,
        };
        let mut file: FileMetadata = self.find(&id)?.timestamped_value.value.clone();
        validate::is_document(&file)?;
        let key = self.decrypt_key(&id, account)?;
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

        Ok((Some(file), document))
    }

    pub fn update_document_unvalidated(
        &mut self, id: &Uuid, document: &[u8], account: &Account,
    ) -> SharedResult<EncryptedDocument> {
        let (op, document) = self.update_document_op(id, document, account)?;
        self.stage_and_promote(op);
        Ok(document)
    }

    pub fn update_document(
        &mut self, id: &Uuid, document: &[u8], account: &Account,
    ) -> SharedResult<EncryptedDocument> {
        let (op, document) = self.update_document_op(id, document, account)?;
        self.stage_validate_and_promote(op, Owner(account.public_key()))?;
        Ok(document)
    }

    pub fn delete_unreferenced_file_versions(&self, config: &Config) -> SharedResult<()> {
        let base_files = self.tree.base.all_files()?.into_iter();
        let local_files = self.tree.all_files()?.into_iter();
        let file_hmacs = base_files
            .chain(local_files)
            .filter_map(|f| f.document_hmac().map(|hmac| (f.id(), hmac)))
            .collect::<HashSet<_>>();
        document_repo::retain(config, file_hmacs)
    }

    // assumptions: no orphans
    // changes: moves files
    // invalidated by: moved files
    // todo: incrementalism
    pub fn unmove_moved_files_in_cycles(
        self, account: &Account,
    ) -> SharedResult<TreeWithOps<Base, Local>> {
        let mut result = self.stage_lazy(Vec::new());

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
                current_file = match result.maybe_find_parent(current_file) {
                    Some(file) => file,
                    None => {
                        if current_file.user_access_keys().is_empty() {
                            return Err(SharedError::FileParentNonexistent);
                        } else {
                            break;
                        }
                    }
                }
            }
            no_cycles_in_ancestors.extend(ancestors);
        }

        for id in to_revert {
            if let (Some(base), Some(_)) =
                (result.tree.base.base.maybe_find(&id), result.tree.base.staged.maybe_find(&id))
            {
                let parent_id = *base.parent();
                // modified version of stage_move where we use keys from base instead of local (which has a cycle)
                // also, we don't care if files are deleted
                result = {
                    let id = &id;
                    let mut file = result.find(id)?.timestamped_value.value.clone();

                    let (key, parent_key) = {
                        let mut local = result.tree.base.to_lazy();
                        let mut base = local.tree.base.to_lazy();
                        let key = base.decrypt_key(id, account)?;
                        let parent_key = base.decrypt_key(&parent_id, account)?;
                        local.tree.base = base.tree;
                        result.tree.base = local.tree;
                        (key, parent_key)
                    };
                    file.parent = parent_id;
                    file.folder_access_key = symkey::encrypt(&parent_key, &key)?;
                    file.name =
                        SecretFileName::from_str(&file.name.to_string(&key)?, &key, &parent_key)?;
                    let file = file.sign(account)?;

                    result.stage_lazy(Some(file))
                }
                .promote();
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
        let mut result = self.stage_lazy(Vec::new());

        for (_, sibling_ids) in result.all_children()?.clone() {
            let mut not_deleted_sibling_ids = HashSet::new();
            for id in sibling_ids {
                if !result.calculate_deleted(&id)? {
                    not_deleted_sibling_ids.insert(id);
                }
            }
            for sibling_id in not_deleted_sibling_ids.iter() {
                // todo: check for renames specifically
                if result.tree.base.staged.maybe_find(sibling_id).is_none() {
                    continue;
                }
                let mut name = result.name(sibling_id, account)?;
                let mut changed = false;
                while not_deleted_sibling_ids
                    .iter()
                    .filter(|&id| id != sibling_id)
                    .map(|id| result.name(id, account))
                    .propagate_err()?
                    .any(|sibling_name| sibling_name == name)
                {
                    name = NameComponents::from(&name).generate_next().to_name();
                    changed = true;
                }
                if changed {
                    result.rename_unvalidated(sibling_id, &name, account)?;
                }
            }
        }

        Ok(result)
    }

    pub fn deduplicate_links(self, account: &Account) -> SharedResult<TreeWithOps<Base, Local>> {
        let mut result = self.stage_lazy(Vec::new());

        let mut base_link_targets = HashSet::new();
        for id in result.tree.base.base.owned_ids() {
            if result.calculate_deleted(&id)? {
                continue;
            }
            if let FileType::Link { target } = result.find(&id)?.file_type() {
                base_link_targets.insert(target);
            }
        }

        for id in result.tree.base.staged.owned_ids() {
            if result.calculate_deleted(&id)? {
                continue;
            }
            if let FileType::Link { target } = result.find(&id)?.file_type() {
                if base_link_targets.contains(&target) {
                    result.delete_unvalidated(&id, account)?;
                }
            }
        }
        Ok(result)
    }

    pub fn resolve_shared_links(
        mut self, account: &Account,
    ) -> SharedResult<TreeWithOps<Base, Local>> {
        let mut base_shared_files = HashSet::new();
        let mut base_links = HashSet::new();
        let (base, local_changes) = self.unstage();
        for id in base.owned_ids() {
            let file = base.find(&id)?;
            if file.is_shared() {
                base_shared_files.insert(id);
            }
            if matches!(file.file_type(), FileType::Link { .. }) {
                base_links.insert(id);
            }
        }
        self = base.stage_lazy(local_changes);

        let mut result = self.stage_lazy(Vec::new());
        for id in result.tree.base.staged.owned_ids() {
            if result.find(&id)?.is_shared() {
                for descendant in result.descendants(&id)? {
                    if base_links.contains(&descendant) {
                        // unshare newly shared folder with link inside
                        result.delete_share_unvalidated(&id, None, account)?;
                    }
                }
            }
            if !result.ancestors(&id)?.is_disjoint(&base_shared_files)
                && matches!(result.find(&id)?.file_type(), FileType::Link { .. })
            {
                // delete new link in shared folder
                result.delete_unvalidated(&id, account)?;
            }
        }
        Ok(result)
    }

    pub fn resolve_owned_links(self, account: &Account) -> SharedResult<TreeWithOps<Base, Local>> {
        let mut result = self.stage_lazy(Vec::new());

        for id in result.tree.base.staged.owned_ids() {
            if let FileType::Link { target } = result.find(&id)?.file_type() {
                if result.find(&target)?.owner().0 == account.public_key() {
                    // delete new link to owned file
                    result.delete_unvalidated(&id, account)?;
                }
            }
            if result.find(&id)?.owner().0 == account.public_key() && result.link(&id)?.is_some() {
                // unmove newly owned file with a link targeting it
                let old_parent = *result.tree.base.base.find(&id)?.parent();
                result.move_unvalidated(&id, &old_parent, account)?;
            }
        }
        Ok(result)
    }

    pub fn delete_links_to_deleted_files(
        self, account: &Account,
    ) -> SharedResult<TreeWithOps<Base, Local>> {
        let mut result = self.stage_lazy(Vec::new());

        for id in result.owned_ids() {
            if result.calculate_deleted(&id)? {
                continue;
            }
            let file = result.find(&id)?;
            if let FileType::Link { target } = file.file_type() {
                if result.calculate_deleted(&target)? {
                    // delete link to deleted file file
                    result.delete_unvalidated(&id, account)?;
                }
            }
        }
        Ok(result)
    }
}

impl<Base, Remote, Local> LazyStage2<Base, Remote, Local>
where
    Base: TreeLikeMut<F = SignedFile>,
    Remote: TreeLikeMut<F = Base::F>,
    Local: TreeLikeMut<F = Base::F>,
{
    /// Applies changes to local such that this is a valid tree.
    pub fn merge(
        self, config: &Config, account: &Account, remote_document_changes: &HashSet<Uuid>,
    ) -> SharedResult<Self>
    where
        Base: TreeLikeMut<F = SignedFile>,
        Remote: TreeLikeMut<F = SignedFile>,
        Local: TreeLikeMut<F = SignedFile>,
    {
        let mut result = self.stage_lazy(Vec::new());

        // merge files on an individual basis
        {
            for id in result.tree.base.base.staged.owned_ids() {
                if result.tree.base.staged.maybe_find(&id).is_some() {
                    // 3-way merge
                    if result.tree.base.base.base.maybe_find(&id).is_some() {
                        let (
                            base_file,
                            remote_change,
                            remote_deleted,
                            local_change,
                            parent,
                            name,
                            document_hmac,
                            folder_access_key,
                            user_access_keys,
                        ) = {
                            let (local, merge_changes) = result.unstage();
                            let (remote, local_changes) = local.unstage();
                            let (mut base, remote_changes) = remote.unstage();
                            let base_file = base.find(&id)?.clone();
                            let remote_change = remote_changes.find(&id)?.clone();
                            let local_change = local_changes.find(&id)?.clone();
                            let base_name = base.name(&id, account)?;
                            let mut remote = base.stage_lazy(remote_changes);
                            let remote_name = remote.name(&id, account)?;
                            let remote_deleted = remote.calculate_deleted(&id)?;
                            let mut local = remote.stage_lazy(local_changes);
                            let local_name = local.name(&id, account)?;
                            result = local.stage_lazy(merge_changes);
                            let document_hmac = three_way_merge(
                                &base_file.document_hmac(),
                                &remote_change.document_hmac(),
                                &local_change.document_hmac(),
                                &None, // overwritten during document merge if local != remote
                            )
                            .cloned();
                            let parent = *three_way_merge(
                                base_file.parent(),
                                remote_change.parent(),
                                local_change.parent(),
                                remote_change.parent(),
                            );
                            let key = result.decrypt_key(&id, account)?;
                            let (name, folder_access_key) = {
                                // we may not have the parent of a direct share
                                // in that case changes are unauthorized anyway
                                if result.maybe_find(&parent).is_some() {
                                    let parent_key = result.decrypt_key(&parent, account)?;
                                    let name = SecretFileName::from_str(
                                        three_way_merge(
                                            &base_name,
                                            &remote_name,
                                            &local_name,
                                            &remote_name,
                                        ),
                                        &key,
                                        &parent_key,
                                    )?;
                                    let folder_access_key = symkey::encrypt(&parent_key, &key)?;
                                    (name, folder_access_key)
                                } else {
                                    (
                                        remote_change.secret_name().clone(),
                                        remote_change.folder_access_key().clone(),
                                    )
                                }
                            };
                            let user_access_keys = merge_user_access(
                                Some(base_file.user_access_keys()),
                                remote_change.user_access_keys(),
                                local_change.user_access_keys(),
                            );
                            (
                                base_file,
                                remote_change,
                                remote_deleted,
                                local_change,
                                parent,
                                name,
                                document_hmac,
                                folder_access_key,
                                user_access_keys,
                            )
                        };

                        if remote_deleted {
                            // discard changes to remote-deleted files
                            result.insert(remote_change);
                        } else {
                            result.insert(
                                FileMetadata {
                                    id,
                                    file_type: base_file.file_type(),
                                    parent,
                                    name,
                                    owner: base_file.owner(),
                                    is_deleted: local_change.explicitly_deleted(),
                                    document_hmac,
                                    user_access_keys,
                                    folder_access_key,
                                }
                                .sign(account)?,
                            );
                        }
                    }
                    // 2-way merge
                    else {
                        let (
                            remote_change,
                            remote_name,
                            remote_deleted,
                            local_change,
                            user_access_keys,
                        ) = {
                            let (local, merge_changes) = result.unstage();
                            let (remote, local_changes) = local.unstage();
                            let (base, remote_changes) = remote.unstage();
                            let remote_change = remote_changes.find(&id)?.clone();
                            let local_change = local_changes.find(&id)?.clone();
                            let mut remote = base.stage_lazy(remote_changes);
                            let remote_name = remote.name(&id, account)?;
                            let remote_deleted = remote.calculate_deleted(&id)?;
                            let local = remote.stage_lazy(local_changes);
                            result = local.stage_lazy(merge_changes);
                            let user_access_keys = merge_user_access(
                                None,
                                remote_change.user_access_keys(),
                                local_change.user_access_keys(),
                            );
                            (
                                remote_change,
                                remote_name,
                                remote_deleted,
                                local_change,
                                user_access_keys,
                            )
                        };

                        let key = result.decrypt_key(&id, account)?;
                        let name = {
                            // we may not have the parent of a direct share
                            // in that case changes are unauthorized anyway
                            if result.maybe_find(remote_change.parent()).is_some() {
                                let parent_key =
                                    result.decrypt_key(remote_change.parent(), account)?;
                                SecretFileName::from_str(&remote_name, &key, &parent_key)?
                            } else {
                                remote_change.secret_name().clone()
                            }
                        };

                        if remote_deleted {
                            // discard changes to remote-deleted files
                            result.insert(remote_change);
                        } else {
                            result.insert(
                                FileMetadata {
                                    id,
                                    file_type: remote_change.file_type(),
                                    parent: *remote_change.parent(),
                                    name,
                                    owner: remote_change.owner(),
                                    is_deleted: remote_deleted | local_change.explicitly_deleted(),
                                    document_hmac: remote_change.document_hmac().cloned(), // overwritten during document merge if local != remote
                                    user_access_keys,
                                    folder_access_key: remote_change.folder_access_key().clone(),
                                }
                                .sign(account)?,
                            );
                        }
                    }
                }
            }
        }

        // merge documents
        {
            for id in remote_document_changes {
                let remote_document_change_hmac =
                    result.tree.base.base.staged.find(id)?.document_hmac();
                let remote_document_change =
                    document_repo::get(config, id, remote_document_change_hmac)?;
                if result.calculate_deleted(id)? {
                    // cannot modify locally deleted documents; local changes to deleted documents are reset anyway
                    continue;
                }
                // todo: use merged document type
                let local_document_type =
                    DocumentType::from_file_name_using_extension(&result.name(id, account)?);
                let base_document_hmac = result
                    .tree
                    .base
                    .base
                    .base
                    .maybe_find(id)
                    .and_then(|f| f.document_hmac())
                    .cloned();
                let local_document_hmac = result
                    .tree
                    .base
                    .staged
                    .maybe_find(id)
                    .and_then(|f| f.document_hmac())
                    .cloned();
                let maybe_local_document_change =
                    if local_document_hmac.is_none() || base_document_hmac == local_document_hmac {
                        None
                    } else {
                        Some(document_repo::get(config, id, local_document_hmac.as_ref())?)
                    };
                match (maybe_local_document_change, local_document_type) {
                    // no local changes -> no merge
                    (None, _) => {}
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
                            let decrypted_base_document =
                                document_repo::maybe_get(config, id, base_document_hmac.as_ref())?
                                    .map(|document| base.decrypt_document(id, &document, account))
                                    .map_or(Ok(None), |v| v.map(Some))?
                                    .unwrap_or_default();
                            let mut remote = base.stage_lazy(remote_changes);
                            let decrypted_remote_document =
                                remote.decrypt_document(id, &remote_document_change, account)?;
                            let mut local = remote.stage_lazy(local_changes);
                            let decrypted_local_document =
                                local.decrypt_document(id, &local_document_change, account)?;
                            result = local.stage_lazy(merge_changes);
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
                        let encrypted_document =
                            result.update_document(id, &merged_document, account)?;
                        let hmac = result.find(id)?.document_hmac();
                        document_repo::insert(config, id, hmac, &encrypted_document)?;
                    }
                    // non-text files always duplicated
                    (Some(local_document_change), DocumentType::Drawing | DocumentType::Other) => {
                        let (decrypted_remote_document, decrypted_local_document) = {
                            let (local, merge_changes) = result.unstage();
                            let (mut remote, local_changes) = local.unstage();
                            let decrypted_remote_document =
                                remote.decrypt_document(id, &remote_document_change, account)?;
                            let mut local = remote.stage_lazy(local_changes);
                            let decrypted_local_document =
                                local.decrypt_document(id, &local_document_change, account)?;
                            result = local.stage_lazy(merge_changes);
                            (decrypted_remote_document, decrypted_local_document)
                        };

                        // overwrite existing document (todo: avoid decrypting and re-encrypting document)
                        let encrypted_document = result.update_document_unvalidated(
                            id,
                            &decrypted_remote_document,
                            account,
                        )?;
                        let hmac = result.find(id)?.document_hmac();
                        document_repo::insert(config, id, hmac, &encrypted_document)?;

                        // create copied document (todo: avoid decrypting and re-encrypting document)
                        let (&existing_parent, existing_file_type) = {
                            let existing_document = result.find(id)?;
                            (existing_document.parent(), existing_document.file_type())
                        };

                        let name = result.name(id, account)?;
                        let copied_document_id = result.create_unvalidated(
                            &existing_parent,
                            &name,
                            existing_file_type,
                            account,
                        )?;
                        let encrypted_document = result.update_document_unvalidated(
                            &copied_document_id,
                            &decrypted_local_document,
                            account,
                        )?;
                        let copied_hmac = result.find(&copied_document_id)?.document_hmac();
                        document_repo::insert(
                            config,
                            &copied_document_id,
                            copied_hmac,
                            &encrypted_document,
                        )?;
                    }
                }
            }
        }

        // resolve tree merge conflicts
        let mut result = result.promote();
        result = result.unmove_moved_files_in_cycles(account)?.promote();
        result = result.rename_files_with_path_conflicts(account)?.promote();
        result = result.deduplicate_links(account)?.promote();
        result = result.resolve_shared_links(account)?.promote();
        result = result.resolve_owned_links(account)?.promote();
        result = result.delete_links_to_deleted_files(account)?.promote();

        Ok(result)
    }
}

fn merge_user_access(
    base_user_access: Option<&[UserAccessInfo]>, remote_user_access: &[UserAccessInfo],
    local_user_access: &[UserAccessInfo],
) -> Vec<UserAccessInfo> {
    let mut user_access_keys = HashMap::<Owner, UserAccessInfo>::new();
    for user_access in base_user_access
        .unwrap_or(&[])
        .iter()
        .chain(remote_user_access.iter())
        .chain(local_user_access.iter())
    {
        if let Some(mut existing_user_access) =
            user_access_keys.remove(&Owner(user_access.encrypted_for))
        {
            if user_access.deleted {
                existing_user_access.deleted = true;
                user_access_keys
                    .insert(Owner(existing_user_access.encrypted_for), existing_user_access);
            } else if user_access.mode >= existing_user_access.mode {
                user_access_keys.insert(Owner(user_access.encrypted_for), user_access.clone());
            } else {
                user_access_keys
                    .insert(Owner(existing_user_access.encrypted_for), existing_user_access);
            }
        } else {
            user_access_keys.insert(Owner(user_access.encrypted_for), user_access.clone());
        }
    }
    user_access_keys.into_values().into_iter().collect()
}

/// Returns the 3-way merge of any comparable value; returns `resolution` in the event of a conflict.
///
/// # Examples
///
/// ```ignore
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
