use db_rs::LookupTable;
use std::collections::HashSet;

use hmac::{Mac, NewMac};
use libsecp256k1::PublicKey;
use tracing::debug;
use uuid::Uuid;

use crate::access_info::{UserAccessInfo, UserAccessMode};
use crate::account::Account;
use crate::core_config::Config;
use crate::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::file::{File, Share, ShareMode};
use crate::file_like::FileLike;
use crate::file_metadata::{FileMetadata, FileType, Owner};
use crate::lazy::LazyTree;
use crate::secret_filename::{HmacSha256, SecretFileName};
use crate::signed_file::SignedFile;
use crate::staged::{StagedTree, StagedTreeLike};
use crate::tree_like::{TreeLike, TreeLikeMut};
use crate::{compression_service, document_repo, symkey, validate, SharedErrorKind, SharedResult};

pub type TreeWithOp<Staged> = LazyTree<StagedTree<Staged, Option<SignedFile>>>;
pub type TreeWithOps<Staged> = LazyTree<StagedTree<Staged, Vec<SignedFile>>>;

impl<T> LazyTree<T>
where
    T: TreeLike<F = SignedFile>,
{
    ///  decrypt file fields in preparation for converting FileMetadata into File
    fn decrypt(
        &mut self, id: &Uuid, account: &Account, public_key_cache: &mut LookupTable<Owner, String>,
    ) -> SharedResult<File> {
        let meta = self.find(id)?.clone();
        let file_type = meta.file_type();
        let last_modified = meta.timestamped_value.timestamp as u64;
        let name = self.name_using_links(id, account)?;
        let parent = *meta.parent();
        let last_modified_by = account.username.clone();
        let id = *id;

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
                        .data()
                        .get(&Owner(user_access_key.encrypted_by))
                        .cloned()
                        .unwrap_or_else(|| String::from("<unknown>"))
                },
                shared_with: if user_access_key.encrypted_for == account.public_key() {
                    account.username.clone()
                } else {
                    public_key_cache
                        .data()
                        .get(&Owner(user_access_key.encrypted_for))
                        .cloned()
                        .unwrap_or_else(|| String::from("<unknown>"))
                },
            });
        }

        Ok(File { id, parent, name, file_type, last_modified, last_modified_by, shares })
    }

    /// convert FileMetadata into File. fields have been decrypted, public keys replaced with usernames, deleted files filtered out, etc.
    pub fn finalize(
        &mut self, account: &Account, id: Uuid, public_key_cache: &mut LookupTable<Owner, String>,
    ) -> SharedResult<File> {
        let finalized = self.decrypt(&id, account, public_key_cache)?;

        let file = match finalized.file_type {
            FileType::Document | FileType::Folder => finalized,
            FileType::Link { target } => {
                let mut target_file = self.decrypt(&target, account, public_key_cache)?;
                target_file.parent = finalized.parent;

                target_file
            }
        };

        Ok(file)
    }

    pub fn finalize_all<I>(
        &mut self, account: &Account, ids: I, public_key_cache: &mut LookupTable<Owner, String>,
        skip_invisible: bool,
    ) -> SharedResult<Vec<File>>
    where
        I: Iterator<Item = Uuid>,
    {
        let mut files: Vec<File> = Vec::new();

        for id in ids {
            if skip_invisible && self.is_invisible_id(id)? {
                continue;
            }

            let finalized = self.decrypt(&id, account, public_key_cache)?;

            let file = match finalized.file_type {
                FileType::Document | FileType::Folder => finalized,
                FileType::Link { target } => {
                    let mut target_file = self.decrypt(&target, account, public_key_cache)?;
                    target_file.parent = finalized.parent;

                    target_file
                }
            };
            files.push(file);
        }

        Ok(files)
    }

    fn is_invisible_id(&mut self, id: Uuid) -> SharedResult<bool> {
        Ok(self.calculate_deleted(&id)? || self.in_pending_share(&id)? || self.link(&id)?.is_some())
    }

    pub fn create_op(
        &mut self, id: Uuid, key: AESKey, parent: &Uuid, name: &str, file_type: FileType,
        account: &Account,
    ) -> SharedResult<(SignedFile, Uuid)> {
        validate::file_name(name)?;

        if self.maybe_find(parent).is_none() {
            return Err(SharedErrorKind::FileParentNonexistent.into());
        }
        let parent_owner = self.find(parent)?.owner().0;
        let parent_key = self.decrypt_key(parent, account)?;
        let file =
            FileMetadata::create(id, key, &parent_owner, *parent, &parent_key, name, file_type)?
                .sign(account)?;
        let id = *file.id();

        debug!("new {:?} with id: {}", file_type, id);
        Ok((file, id))
    }

    pub fn rename_op(
        &mut self, id: &Uuid, name: &str, account: &Account,
    ) -> SharedResult<SignedFile> {
        let mut file = self.find(id)?.timestamped_value.value.clone();

        validate::file_name(name)?;
        if self.maybe_find(file.parent()).is_none() {
            return Err(SharedErrorKind::InsufficientPermission.into());
        }
        let parent_key = self.decrypt_key(file.parent(), account)?;
        let key = self.decrypt_key(id, account)?;
        file.name = SecretFileName::from_str(name, &key, &parent_key)?;
        let file = file.sign(account)?;

        Ok(file)
    }

    pub fn move_op(
        &mut self, id: &Uuid, new_parent: &Uuid, account: &Account,
    ) -> SharedResult<Vec<SignedFile>> {
        let mut file = self.find(id)?.timestamped_value.value.clone();
        if self.maybe_find(new_parent).is_none() {
            return Err(SharedErrorKind::FileParentNonexistent.into());
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
            if self.calculate_deleted(&id)? {
                continue;
            }
            let mut descendant = self.find(&id)?.timestamped_value.value.clone();
            descendant.owner = owner;
            result.push(descendant.sign(account)?);
        }

        Ok(result)
    }

    pub fn delete_op(&self, id: &Uuid, account: &Account) -> SharedResult<SignedFile> {
        let mut file = self.find(id)?.timestamped_value.value.clone();

        file.is_deleted = true;
        let file = file.sign(account)?;

        Ok(file)
    }

    pub fn add_share_op(
        &mut self, id: Uuid, sharee: Owner, mode: ShareMode, account: &Account,
    ) -> SharedResult<SignedFile> {
        let owner = Owner(account.public_key());
        let access_mode = match mode {
            ShareMode::Write => UserAccessMode::Write,
            ShareMode::Read => UserAccessMode::Read,
        };
        if self.calculate_deleted(&id)? {
            return Err(SharedErrorKind::FileNonexistent.into());
        }
        let id =
            if let FileType::Link { target } = self.find(&id)?.file_type() { target } else { id };
        let mut file = self.find(&id)?.timestamped_value.value.clone();
        validate::not_root(&file)?;
        if mode == ShareMode::Write && file.owner.0 != owner.0 {
            return Err(SharedErrorKind::InsufficientPermission.into());
        }
        // check for and remove duplicate shares
        let mut found = false;
        for user_access in &mut file.user_access_keys {
            if user_access.encrypted_for == sharee.0 {
                found = true;
                if user_access.mode == access_mode && !user_access.deleted {
                    return Err(SharedErrorKind::DuplicateShare.into());
                }
            }
        }
        if found {
            file.user_access_keys
                .retain(|k| k.encrypted_for != sharee.0);
        }
        file.user_access_keys.push(UserAccessInfo::encrypt(
            account,
            &owner.0,
            &sharee.0,
            &self.decrypt_key(&id, account)?,
            access_mode,
        )?);
        let file = file.sign(account)?;

        Ok(file)
    }

    pub fn delete_share_op(
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
            return Err(SharedErrorKind::ShareNonexistent.into());
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

    pub fn read_document(
        &mut self, config: &Config, id: &Uuid, account: &Account,
    ) -> SharedResult<DecryptedDocument> {
        if self.calculate_deleted(id)? {
            return Err(SharedErrorKind::FileNonexistent.into());
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
            None => return Err(SharedErrorKind::FileNonexistent.into()),
        };

        Ok(doc)
    }

    pub fn update_document_op(
        &mut self, id: &Uuid, document: &[u8], account: &Account,
    ) -> SharedResult<(SignedFile, EncryptedDocument)> {
        let id = match self.find(id)?.file_type() {
            FileType::Document | FileType::Folder => *id,
            FileType::Link { target } => target,
        };
        let mut file: FileMetadata = self.find(&id)?.timestamped_value.value.clone();
        validate::is_document(&file)?;
        let key = self.decrypt_key(&id, account)?;
        let hmac = {
            let mut mac =
                HmacSha256::new_from_slice(&key).map_err(SharedErrorKind::HmacCreationError)?;
            mac.update(document);
            mac.finalize().into_bytes()
        }
        .into();
        file.document_hmac = Some(hmac);
        let file = file.sign(account)?;
        let document = compression_service::compress(document)?;
        let document = symkey::encrypt(&key, &document)?;

        Ok((file, document))
    }
}

impl<Base, Local, Staged> LazyTree<Staged>
where
    Staged: StagedTreeLike<Base = Base, Staged = Local, F = SignedFile> + TreeLikeMut,
    Base: TreeLike<F = Staged::F>,
    Local: TreeLikeMut<F = Staged::F>,
{
    pub fn create_unvalidated(
        &mut self, id: Uuid, key: AESKey, parent: &Uuid, name: &str, file_type: FileType,
        account: &Account,
    ) -> SharedResult<Uuid> {
        let (op, id) = self.create_op(id, key, parent, name, file_type, account)?;
        self.stage_and_promote(Some(op))?;
        Ok(id)
    }

    pub fn create(
        &mut self, id: Uuid, key: AESKey, parent: &Uuid, name: &str, file_type: FileType,
        account: &Account,
    ) -> SharedResult<Uuid> {
        if self.calculate_deleted(parent)? {
            return Err(SharedErrorKind::FileParentNonexistent.into());
        }

        let (op, id) = self.create_op(id, key, parent, name, file_type, account)?;
        self.stage_validate_and_promote(Some(op), Owner(account.public_key()))?;
        Ok(id)
    }

    pub fn rename_unvalidated(
        &mut self, id: &Uuid, name: &str, account: &Account,
    ) -> SharedResult<()> {
        let op = self.rename_op(id, name, account)?;
        self.stage_and_promote(Some(op))?;
        Ok(())
    }

    pub fn rename(&mut self, id: &Uuid, name: &str, account: &Account) -> SharedResult<()> {
        let op = self.rename_op(id, name, account)?;
        self.stage_validate_and_promote(Some(op), Owner(account.public_key()))?;
        Ok(())
    }

    pub fn move_unvalidated(
        &mut self, id: &Uuid, new_parent: &Uuid, account: &Account,
    ) -> SharedResult<()> {
        let op = self.move_op(id, new_parent, account)?;
        self.stage_and_promote(op)?;
        Ok(())
    }

    pub fn move_file(
        &mut self, id: &Uuid, new_parent: &Uuid, account: &Account,
    ) -> SharedResult<()> {
        if self.maybe_find(new_parent).is_none() || self.calculate_deleted(new_parent)? {
            return Err(SharedErrorKind::FileParentNonexistent.into());
        }
        let op = self.move_op(id, new_parent, account)?;
        self.stage_validate_and_promote(op, Owner(account.public_key()))?;
        Ok(())
    }

    pub fn delete_unvalidated(&mut self, id: &Uuid, account: &Account) -> SharedResult<()> {
        let op = self.delete_op(id, account)?;
        self.stage_and_promote(Some(op))?;
        Ok(())
    }

    pub fn delete(&mut self, id: &Uuid, account: &Account) -> SharedResult<()> {
        let op = self.delete_op(id, account)?;
        self.stage_validate_and_promote(Some(op), Owner(account.public_key()))?;
        Ok(())
    }

    pub fn add_share_unvalidated(
        &mut self, id: Uuid, sharee: Owner, mode: ShareMode, account: &Account,
    ) -> SharedResult<()> {
        let op = self.add_share_op(id, sharee, mode, account)?;
        self.stage_and_promote(Some(op))?;
        Ok(())
    }

    pub fn add_share(
        &mut self, id: Uuid, sharee: Owner, mode: ShareMode, account: &Account,
    ) -> SharedResult<()> {
        let op = self.add_share_op(id, sharee, mode, account)?;
        self.stage_validate_and_promote(Some(op), Owner(account.public_key()))?;
        Ok(())
    }

    pub fn delete_share_unvalidated(
        &mut self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>, account: &Account,
    ) -> SharedResult<()> {
        let op = self.delete_share_op(id, maybe_encrypted_for, account)?;
        self.stage_and_promote(op)?;
        Ok(())
    }

    pub fn delete_share(
        &mut self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>, account: &Account,
    ) -> SharedResult<()> {
        let op = self.delete_share_op(id, maybe_encrypted_for, account)?;
        self.stage_validate_and_promote(op, Owner(account.public_key()))?;
        Ok(())
    }

    pub fn update_document_unvalidated(
        &mut self, id: &Uuid, document: &[u8], account: &Account,
    ) -> SharedResult<EncryptedDocument> {
        let (op, document) = self.update_document_op(id, document, account)?;
        self.stage_and_promote(Some(op))?;
        Ok(document)
    }

    pub fn update_document(
        &mut self, id: &Uuid, document: &[u8], account: &Account,
    ) -> SharedResult<EncryptedDocument> {
        let (op, document) = self.update_document_op(id, document, account)?;
        self.stage_validate_and_promote(Some(op), Owner(account.public_key()))?;
        Ok(document)
    }

    pub fn delete_unreferenced_file_versions(&self, config: &Config) -> SharedResult<()> {
        let base_files = self.tree.base().all_files()?.into_iter();
        let local_files = self.tree.all_files()?.into_iter();
        let file_hmacs = base_files
            .chain(local_files)
            .filter_map(|f| f.document_hmac().map(|hmac| (f.id(), hmac)))
            .collect::<HashSet<_>>();
        document_repo::retain(config, file_hmacs)
    }
}
