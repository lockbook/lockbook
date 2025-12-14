use crate::model::access_info::{UserAccessInfo, UserAccessMode};
use crate::model::api::METADATA_FEE;
use crate::model::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file::{File, Share, ShareMode};
use crate::model::file_metadata::{FileType, Owner};
use crate::model::lazy::LazyTree;
use crate::model::meta::Meta;
use crate::model::secret_filename::{HmacSha256, SecretFileName};
use crate::model::staged::{StagedTree, StagedTreeLike};
use crate::model::tree_like::{TreeLike, TreeLikeMut};
use crate::model::{compression_service, symkey, validate};
use crate::service::keychain::Keychain;
use db_rs::LookupTable;
use hmac::{Mac, NewMac};
use libsecp256k1::PublicKey;
use tracing::debug;
use uuid::Uuid;

use super::file_like::FileLike;
use super::signed_meta::SignedMeta;

pub type TreeWithOp<Staged> = LazyTree<StagedTree<Staged, Option<SignedMeta>>>;
pub type TreeWithOps<Staged> = LazyTree<StagedTree<Staged, Vec<SignedMeta>>>;

impl<T> LazyTree<T>
where
    T: TreeLike<F = SignedMeta>,
{
    /// convert FileMetadata into File. fields have been decrypted, public keys replaced with usernames, deleted files filtered out, etc.
    pub fn decrypt(
        &mut self, keychain: &Keychain, id: &Uuid, public_key_cache: &LookupTable<Owner, String>,
    ) -> LbResult<File> {
        let account = keychain.get_account()?;
        let pk = keychain.get_pk()?;

        let meta = self.find(id)?.clone();
        let file_type = meta.file_type();
        let last_modified = meta.timestamped_value.timestamp as u64;
        let name = self.name_using_links(id, keychain)?;
        let parent = self.parent_using_links(id)?;
        let last_modified_by = public_key_cache
            .get()
            .get(&Owner(meta.public_key))
            .cloned()
            .unwrap_or_else(|| String::from("<unknown>"));

        let owner = meta.owner();
        let owner_username = public_key_cache
            .get()
            .get(&owner)
            .cloned()
            .unwrap_or_else(|| String::from("<unknown>"));

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
                shared_by: if user_access_key.encrypted_by == pk {
                    account.username.clone()
                } else {
                    public_key_cache
                        .get()
                        .get(&Owner(user_access_key.encrypted_by))
                        .cloned()
                        .unwrap_or_else(|| String::from("<unknown>"))
                },
                shared_with: if user_access_key.encrypted_for == pk {
                    account.username.clone()
                } else {
                    public_key_cache
                        .get()
                        .get(&Owner(user_access_key.encrypted_for))
                        .cloned()
                        .unwrap_or_else(|| String::from("<unknown>"))
                },
            });
        }

        Ok(File {
            id,
            parent,
            name,
            file_type,
            last_modified,
            last_modified_by,
            owner: owner_username,
            shares,
            size_bytes: meta.timestamped_value.value.doc_size().unwrap_or_default() as u64
                + METADATA_FEE,
        })
    }

    /// convert FileMetadata into File. fields have been decrypted, public keys replaced with usernames, deleted files filtered out, etc.
    pub fn decrypt_all<I>(
        &mut self, keychain: &Keychain, ids: I, public_key_cache: &LookupTable<Owner, String>,
        skip_invisible: bool,
    ) -> LbResult<Vec<File>>
    where
        I: Iterator<Item = Uuid>,
    {
        let mut files: Vec<File> = Vec::new();

        for id in ids {
            if skip_invisible && self.is_invisible_id(id)? {
                continue;
            }

            let finalized = self.decrypt(keychain, &id, public_key_cache)?;
            files.push(finalized);
        }

        Ok(files)
    }

    pub fn is_invisible_id(&mut self, id: Uuid) -> LbResult<bool> {
        Ok(self.find(&id)?.is_link()
            || self.calculate_deleted(&id)?
            || self.in_pending_share(&id)?)
    }

    pub fn create_op(
        &mut self, id: Uuid, key: AESKey, parent: &Uuid, name: &str, file_type: FileType,
        keychain: &Keychain,
    ) -> LbResult<(SignedMeta, Uuid)> {
        validate::file_name(name)?;

        if self.maybe_find(parent).is_none() {
            return Err(LbErrKind::FileParentNonexistent.into());
        }
        let parent_owner = self.find(parent)?.owner().0;
        let parent_key = self.decrypt_key(parent, keychain)?;
        let file = Meta::create(id, key, &parent_owner, *parent, &parent_key, name, file_type)?
            .sign(keychain)?;
        let id = *file.id();

        debug!("new {:?} with id: {}", file_type, id);
        Ok((file, id))
    }

    pub fn rename_op(
        &mut self, id: &Uuid, name: &str, keychain: &Keychain,
    ) -> LbResult<SignedMeta> {
        let mut file = self.find(id)?.timestamped_value.value.clone();

        validate::file_name(name)?;
        if self.maybe_find(file.parent()).is_none() {
            return Err(LbErrKind::InsufficientPermission.into());
        }
        let parent_key = self.decrypt_key(file.parent(), keychain)?;
        let key = self.decrypt_key(id, keychain)?;
        file.set_name(SecretFileName::from_str(name, &key, &parent_key)?);
        let file = file.sign(keychain)?;

        Ok(file)
    }

    pub fn move_op(
        &mut self, id: &Uuid, new_parent: &Uuid, keychain: &Keychain,
    ) -> LbResult<Vec<SignedMeta>> {
        let mut file = self.find(id)?.timestamped_value.value.clone();
        if self.maybe_find(new_parent).is_none() {
            return Err(LbErrKind::FileParentNonexistent.into());
        }
        let key = self.decrypt_key(id, keychain)?;
        let parent_key = self.decrypt_key(new_parent, keychain)?;
        let owner = self.find(new_parent)?.owner();
        file.set_owner(owner);
        file.set_parent(*new_parent);
        file.set_folder_access_keys(symkey::encrypt(&parent_key, &key)?);
        file.set_name(SecretFileName::from_str(&self.name(id, keychain)?, &key, &parent_key)?);
        let file = file.sign(keychain)?;

        let mut result = vec![file];
        for id in self.descendants(id)? {
            if self.calculate_deleted(&id)? {
                continue;
            }
            let mut descendant = self.find(&id)?.timestamped_value.value.clone();
            descendant.set_owner(owner);
            result.push(descendant.sign(keychain)?);
        }

        Ok(result)
    }

    pub fn delete_op(&self, id: &Uuid, keychain: &Keychain) -> LbResult<SignedMeta> {
        let mut file = self.find(id)?.timestamped_value.value.clone();

        file.set_deleted(true);
        let file = file.sign(keychain)?;

        Ok(file)
    }

    pub fn add_share_op(
        &mut self, id: Uuid, sharee: Owner, mode: ShareMode, keychain: &Keychain,
    ) -> LbResult<SignedMeta> {
        let owner = Owner(keychain.get_pk()?);
        let access_mode = match mode {
            ShareMode::Write => UserAccessMode::Write,
            ShareMode::Read => UserAccessMode::Read,
        };
        if self.calculate_deleted(&id)? {
            return Err(LbErrKind::FileNonexistent.into());
        }
        let id =
            if let FileType::Link { target } = self.find(&id)?.file_type() { target } else { id };
        let mut file = self.find(&id)?.timestamped_value.value.clone();
        validate::not_root(&file)?;
        if mode == ShareMode::Write && file.owner().0 != owner.0 {
            return Err(LbErrKind::InsufficientPermission.into());
        }
        // check for and remove duplicate shares
        let mut found = false;
        for user_access in file.user_access_keys() {
            if user_access.encrypted_for == sharee.0 {
                found = true;
                if user_access.mode == access_mode && !user_access.deleted {
                    return Err(LbErrKind::ShareAlreadyExists.into());
                }
            }
        }
        if found {
            file.user_access_keys_mut()
                .retain(|k| k.encrypted_for != sharee.0);
        }
        file.user_access_keys_mut().push(UserAccessInfo::encrypt(
            keychain.get_account()?,
            &owner.0,
            &sharee.0,
            &self.decrypt_key(&id, keychain)?,
            access_mode,
        )?);
        let file = file.sign(keychain)?;

        Ok(file)
    }

    pub fn delete_share_op(
        &mut self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>, keychain: &Keychain,
    ) -> LbResult<Vec<SignedMeta>> {
        let mut result = Vec::new();
        let mut file = self.find(id)?.timestamped_value.value.clone();

        let mut found = false;
        for key in file.user_access_keys_mut() {
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
            return Err(LbErrKind::ShareNonexistent.into());
        }
        result.push(file.sign(keychain)?);

        // delete any links pointing to file
        if let Some(encrypted_for) = maybe_encrypted_for {
            if encrypted_for == keychain.get_pk()? {
                if let Some(link) = self.linked_by(id)? {
                    let mut link = self.find(&link)?.timestamped_value.value.clone();
                    link.set_deleted(true);
                    result.push(link.sign(keychain)?);
                }
            }
        }

        Ok(result)
    }

    pub fn decrypt_document(
        &mut self, id: &Uuid, doc: &EncryptedDocument, keychain: &Keychain,
    ) -> LbResult<DecryptedDocument> {
        let key = self.decrypt_key(id, keychain)?;
        let compressed = symkey::decrypt(&key, doc)?;
        let doc = compression_service::decompress(&compressed)?;

        Ok(doc)
    }

    pub fn update_document_op(
        &mut self, id: &Uuid, document: &[u8], keychain: &Keychain,
    ) -> LbResult<(SignedMeta, EncryptedDocument)> {
        let id = match self.find(id)?.file_type() {
            FileType::Document | FileType::Folder => *id,
            FileType::Link { target } => target,
        };
        let mut file = self.find(&id)?.timestamped_value.value.clone();
        validate::is_document(&file)?;
        let key = self.decrypt_key(&id, keychain)?;
        let hmac = {
            let mut mac = HmacSha256::new_from_slice(&key)
                .map_err(|err| LbErrKind::Unexpected(format!("hmac creation error: {err:?}")))?;
            mac.update(document);
            mac.finalize().into_bytes()
        }
        .into();
        let document = compression_service::compress(document)?;
        let document = symkey::encrypt(&key, &document)?;
        file.set_hmac_and_size(Some(hmac), Some(document.value.len()));
        let file = file.sign(keychain)?;

        Ok((file, document))
    }
}

impl<Base, Local, Staged> LazyTree<Staged>
where
    Staged: StagedTreeLike<Base = Base, Staged = Local, F = SignedMeta> + TreeLikeMut,
    Base: TreeLike<F = Staged::F>,
    Local: TreeLikeMut<F = Staged::F>,
{
    pub fn create_unvalidated(
        &mut self, id: Uuid, key: AESKey, parent: &Uuid, name: &str, file_type: FileType,
        keychain: &Keychain,
    ) -> LbResult<Uuid> {
        let (op, id) = self.create_op(id, key, parent, name, file_type, keychain)?;
        self.stage_and_promote(Some(op))?;
        Ok(id)
    }

    pub fn create(
        &mut self, id: Uuid, key: AESKey, parent: &Uuid, name: &str, file_type: FileType,
        keychain: &Keychain,
    ) -> LbResult<Uuid> {
        if self.calculate_deleted(parent)? {
            return Err(LbErrKind::FileParentNonexistent.into());
        }

        let (op, id) = self.create_op(id, key, parent, name, file_type, keychain)?;
        self.stage_validate_and_promote(Some(op), Owner(keychain.get_pk()?))?;
        Ok(id)
    }

    pub fn rename_unvalidated(
        &mut self, id: &Uuid, name: &str, keychain: &Keychain,
    ) -> LbResult<()> {
        let op = self.rename_op(id, name, keychain)?;
        self.stage_and_promote(Some(op))?;
        Ok(())
    }

    pub fn rename(&mut self, id: &Uuid, name: &str, keychain: &Keychain) -> LbResult<()> {
        let op = self.rename_op(id, name, keychain)?;
        self.stage_validate_and_promote(Some(op), Owner(keychain.get_pk()?))?;
        Ok(())
    }

    pub fn move_unvalidated(
        &mut self, id: &Uuid, new_parent: &Uuid, keychain: &Keychain,
    ) -> LbResult<()> {
        let op = self.move_op(id, new_parent, keychain)?;
        self.stage_and_promote(op)?;
        Ok(())
    }

    pub fn move_file(&mut self, id: &Uuid, new_parent: &Uuid, keychain: &Keychain) -> LbResult<()> {
        if self.maybe_find(new_parent).is_none() || self.calculate_deleted(new_parent)? {
            return Err(LbErrKind::FileParentNonexistent.into());
        }
        let op = self.move_op(id, new_parent, keychain)?;
        self.stage_validate_and_promote(op, Owner(keychain.get_pk()?))?;
        Ok(())
    }

    pub fn delete_unvalidated(&mut self, id: &Uuid, keychain: &Keychain) -> LbResult<()> {
        let op = self.delete_op(id, keychain)?;
        self.stage_and_promote(Some(op))?;
        Ok(())
    }

    pub fn delete(&mut self, id: &Uuid, keychain: &Keychain) -> LbResult<()> {
        let op = self.delete_op(id, keychain)?;
        self.stage_validate_and_promote(Some(op), Owner(keychain.get_pk()?))?;
        Ok(())
    }

    pub fn add_share_unvalidated(
        &mut self, id: Uuid, sharee: Owner, mode: ShareMode, keychain: &Keychain,
    ) -> LbResult<()> {
        let op = self.add_share_op(id, sharee, mode, keychain)?;
        self.stage_and_promote(Some(op))?;
        Ok(())
    }

    pub fn add_share(
        &mut self, id: Uuid, sharee: Owner, mode: ShareMode, keychain: &Keychain,
    ) -> LbResult<()> {
        let op = self.add_share_op(id, sharee, mode, keychain)?;
        self.stage_validate_and_promote(Some(op), Owner(keychain.get_pk()?))?;
        Ok(())
    }

    pub fn delete_share_unvalidated(
        &mut self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>, keychain: &Keychain,
    ) -> LbResult<()> {
        let op = self.delete_share_op(id, maybe_encrypted_for, keychain)?;
        self.stage_and_promote(op)?;
        Ok(())
    }

    pub fn delete_share(
        &mut self, id: &Uuid, maybe_encrypted_for: Option<PublicKey>, keychain: &Keychain,
    ) -> LbResult<()> {
        let op = self.delete_share_op(id, maybe_encrypted_for, keychain)?;
        self.stage_validate_and_promote(op, Owner(keychain.get_pk()?))?;
        Ok(())
    }

    pub fn update_document_unvalidated(
        &mut self, id: &Uuid, document: &[u8], keychain: &Keychain,
    ) -> LbResult<EncryptedDocument> {
        let (op, document) = self.update_document_op(id, document, keychain)?;
        self.stage_and_promote(Some(op))?;
        Ok(document)
    }

    pub fn update_document(
        &mut self, id: &Uuid, document: &[u8], keychain: &Keychain,
    ) -> LbResult<EncryptedDocument> {
        let (op, document) = self.update_document_op(id, document, keychain)?;
        self.stage_validate_and_promote(Some(op), Owner(keychain.get_pk()?))?;
        Ok(document)
    }
}
