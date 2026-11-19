use crate::model::access_info::{UserAccessInfo, UserAccessMode};
use crate::model::api::METADATA_FEE;
use crate::model::crypto::{AESKey, DecryptedDocument, EncryptedDocument};
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file::{File, Share, ShareMode};
use crate::model::file_metadata::{DocumentHmac, FileType, Owner};
use crate::model::filename::NameComponents;
use crate::model::lazy::LazyTree;
use crate::model::meta::{AttachmentAccessInfo, Meta};
use crate::model::secret_filename::{HmacSha256, SecretFileName};
use crate::model::staged::{StagedTree, StagedTreeLike};
use crate::model::tree_like::{TreeLike, TreeLikeMut};
use crate::model::{compression_service, symkey, validate};
use crate::service::keychain::Keychain;
use db_rs::views::hashmap::DbHashMap;
use hmac::{Mac, NewMac};
use libsecp256k1::PublicKey;
use std::collections::HashSet;
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
        &mut self, keychain: &Keychain, id: &Uuid, public_key_cache: &DbHashMap<Owner, String>,
    ) -> LbResult<File> {
        let account = keychain.get_account()?;
        let pk = keychain.get_pk()?;

        let meta = self.find(id)?.clone();
        let file_type = meta.file_type();
        let last_modified = meta.timestamped_value.timestamp as u64;
        let name = self.name_using_links(id, keychain)?;
        let parent = self.parent_using_links(id)?;
        let last_modified_by = public_key_cache
            .get(&Owner(meta.public_key))
            .cloned()
            .unwrap_or_else(|| String::from("<unknown>"));

        let owner = meta.owner();
        let owner_username = public_key_cache
            .get(&owner)
            .cloned()
            .unwrap_or_else(|| String::from("<unknown>"));

        let id = *id;

        let mut shares = Vec::new();
        for user_access_key in meta.user_access_keys() {
            // a share to the file's owner, which a move into the recipient's folder carries along,
            // grants nothing
            if user_access_key.encrypted_by == user_access_key.encrypted_for
                || user_access_key.encrypted_for == meta.owner().0
                || user_access_key.deleted
            {
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
                        .get(&Owner(user_access_key.encrypted_by))
                        .cloned()
                        .unwrap_or_else(|| String::from("<unknown>"))
                },
                shared_with: if user_access_key.encrypted_for == pk {
                    account.username.clone()
                } else {
                    public_key_cache
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
        &mut self, keychain: &Keychain, ids: I, public_key_cache: &DbHashMap<Owner, String>,
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

    #[allow(clippy::too_many_arguments)]
    pub fn create_op(
        &mut self, id: Uuid, key: AESKey, parent: &Uuid, name: &str, file_type: FileType,
        attachment_of: Option<AttachmentAccessInfo>, keychain: &Keychain,
    ) -> LbResult<(SignedMeta, Uuid)> {
        validate::file_name(name)?;

        if self.maybe_find(parent).is_none() {
            return Err(LbErrKind::FileParentNonexistent.into());
        }
        let parent_owner = self.find(parent)?.owner().0;
        let parent_key = self.decrypt_key(parent, keychain)?;
        let file = Meta::create(
            id,
            key,
            &parent_owner,
            *parent,
            &parent_key,
            name,
            file_type,
            attachment_of,
        )?
        .sign(keychain)?;
        let id = *file.id();

        debug!("new {:?} with id: {}", file_type, id);
        Ok((file, id))
    }

    /// `document`'s attachments folder, beside it and named for it as it is now.
    pub fn create_attachments_op(
        &mut self, id: Uuid, key: AESKey, document: &Uuid, keychain: &Keychain,
    ) -> LbResult<SignedMeta> {
        let doc = self.find(document)?;
        validate::is_document(doc)?;
        let parent = *doc.parent();
        if self.maybe_find(&parent).is_none() {
            return Err(LbErrKind::InsufficientPermission.into());
        }

        // Generate a name for a note's attachments folder: `note.md` becomes `note-attachments`
        let stem = NameComponents::from(&self.name(document, keychain)?).stem();
        let name = NameComponents::folder(&format!("{stem}-attachments"));
        let name = self.free_name(&parent, &name, None, keychain)?;

        let document_key = self.decrypt_key(document, keychain)?;
        let attachment_of =
            AttachmentAccessInfo { id: *document, key: symkey::encrypt(&document_key, &key)? };
        let (file, _) = self.create_op(
            id,
            key,
            &parent,
            &name,
            FileType::Folder,
            Some(attachment_of),
            keychain,
        )?;
        Ok(file)
    }

    fn free_name(
        &mut self, parent: &Uuid, name: &NameComponents, except: Option<Uuid>, keychain: &Keychain,
    ) -> LbResult<String> {
        let mut taken = HashSet::new();
        for sibling in self.children(parent)? {
            if Some(sibling) != except && !self.calculate_deleted(&sibling)? {
                taken.insert(self.name(&sibling, keychain)?);
            }
        }
        Ok((1..)
            .map(|n| name.numbered(n))
            .find(|name| !taken.contains(name))
            .unwrap())
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
        let moved = *id;
        for id in self.descendants(&moved)? {
            // deleted after the move: deleted inside the moved file, not above it
            let mut file = self.find(&id)?;
            while !file.explicitly_deleted() && *file.parent() != moved {
                file = self.find(file.parent())?;
            }
            if file.explicitly_deleted() {
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
        if self.access_mode(owner, &id)? < Some(UserAccessMode::Write) {
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
        let unshared = file.sign(keychain)?;
        result.push(unshared.clone());

        // delete my links to the file, and to files in it that are no longer shared with me
        let me = Owner(keychain.get_pk()?);
        if maybe_encrypted_for == Some(me.0) {
            let mut links = vec![];
            for link in self.ids() {
                if let FileType::Link { target } = self.find(&link)?.file_type() {
                    if self.find(&link)?.owner() == me && !self.calculate_deleted(&link)? {
                        links.push((link, target));
                    }
                }
            }
            let after = StagedTree::new(&self.tree, Some(unshared)).to_lazy();
            for (link, target) in links {
                let unreachable = target == *id
                    || (after.maybe_find(&target).is_some()
                        && after.ancestors(&target)?.contains(id)
                        && after.access_mode(me, &target)?.is_none());
                if unreachable {
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

    pub fn overwrite_document_hmac_op(
        &mut self, id: &Uuid, new_hmac: Option<DocumentHmac>, new_size: Option<usize>,
        keychain: &Keychain,
    ) -> LbResult<SignedMeta> {
        let id = match self.find(id)?.file_type() {
            FileType::Document | FileType::Folder => *id,
            FileType::Link { target } => target,
        };
        let mut file = self.find(&id)?.timestamped_value.value.clone();
        validate::is_document(&file)?;
        file.set_hmac_and_size(new_hmac, new_size);
        file.sign(keychain)
    }
}

impl<Base, Local, Staged> LazyTree<Staged>
where
    Staged: StagedTreeLike<Base = Base, Staged = Local, F = SignedMeta> + TreeLikeMut,
    Base: TreeLike<F = Staged::F>,
    Local: TreeLikeMut<F = Staged::F>,
{
    #[allow(clippy::too_many_arguments)]
    pub fn create_unvalidated(
        &mut self, id: Uuid, key: AESKey, parent: &Uuid, name: &str, file_type: FileType,
        attachment_of: Option<AttachmentAccessInfo>, keychain: &Keychain,
    ) -> LbResult<Uuid> {
        let (op, id) = self.create_op(id, key, parent, name, file_type, attachment_of, keychain)?;
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

        let (op, id) = self.create_op(id, key, parent, name, file_type, None, keychain)?;
        self.stage_validate_and_promote(Some(op), Owner(keychain.get_pk()?))?;
        Ok(id)
    }

    /// Creates `document`'s attachments folder beside it.
    pub fn create_attachments(
        &mut self, id: Uuid, key: AESKey, document: &Uuid, keychain: &Keychain,
    ) -> LbResult<Uuid> {
        if self.calculate_deleted(document)? {
            return Err(LbErrKind::FileNonexistent.into());
        }
        let op = self.create_attachments_op(id, key, document, keychain)?;
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

    /// Renames `id`. A document's attachments folder keeps its name, and can't be renamed itself.
    pub fn rename(&mut self, id: &Uuid, name: &str, keychain: &Keychain) -> LbResult<()> {
        if self.find(id)?.attachment_of().is_some() {
            return Err(LbErrKind::AttachmentsFolderFixed.into());
        }
        let op = self.rename_op(id, name, keychain)?;
        self.stage_validate_and_promote(Some(op), Owner(keychain.get_pk()?))?;
        Ok(())
    }

    /// Whether a document should have attachments, such as upon the first write share
    pub fn wants_attachments(&mut self, id: &Uuid, me: Owner) -> LbResult<bool> {
        let file = self.find(id)?;
        let written = file
            .user_access_keys()
            .iter()
            .any(|k| !k.deleted && k.mode == UserAccessMode::Write && k.encrypted_for != me.0);
        Ok(file.is_document()
            && file.owner() == me
            && written
            && !self.calculate_deleted(id)?
            && self.attachments_folder(id)?.is_none())
    }

    /// Puts an attachments folder beside its target whether during a move or during a sync merge
    /// repair
    pub fn move_attachments_beside_note(
        &mut self, document: &Uuid, folder: &Uuid, keychain: &Keychain,
    ) -> LbResult<()> {
        let folder = *folder;
        let parent = *self.find(document)?.parent();
        self.move_unvalidated(&folder, &parent, keychain)?;
        let name = self.name(&folder, keychain)?;
        let free =
            self.free_name(&parent, &NameComponents::folder(&name), Some(folder), keychain)?;
        if free != name {
            self.rename_unvalidated(&folder, &free, keychain)?;
        }
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
        if self.find(id)?.attachment_of().is_some() {
            return Err(LbErrKind::AttachmentsFolderFixed.into());
        }
        let mut tree = StagedTree::new(&self.tree, Vec::new()).to_lazy();
        tree.move_unvalidated(id, new_parent, keychain)?;
        if let Some(folder) = tree.attachments_folder(id)? {
            tree.move_attachments_beside_note(id, &folder, keychain)?;
        }
        let (_, ops) = tree.unstage();
        self.stage_validate_and_promote(ops, Owner(keychain.get_pk()?))?;
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
        let me = Owner(keychain.get_pk()?);
        let mut tree = StagedTree::new(&self.tree, Vec::new()).to_lazy();
        if maybe_encrypted_for == Some(me.0) {
            // this device is leaving the share: its moves into the share that never reached the
            // server go back, as a sync would put them, rather than leaving with the share. e.g.
            // the server has /notes/ and /team/, the share; here notes was moved to /team/notes/;
            // after leaving, notes is back at /notes/
            let synced = self.tree.base().to_lazy();

            // 1. the unsynced moves, as (file, to, from): files whose parent here differs from
            //    the server's, skipping files deleted here and files whose old parent was
            //    deleted here, whose move stays
            let mut moves = Vec::new();
            for moved in self.tree.staged().ids() {
                let Some(base) = synced.maybe_find(&moved) else { continue };
                let file = self.find(&moved)?;
                if !file.explicitly_deleted()
                    && file.parent() != base.parent()
                    && !tree.calculate_deleted(base.parent())?
                {
                    moves.push((moved, *file.parent(), *base.parent()));
                }
            }

            // 2. a move goes back when its new parent is in the share and its old parent isn't.
            //    whether a file is in the share is asked of the server's tree, or of this
            //    device's for a file the server doesn't have. a new parent must be in the share
            //    here too: one this device took out of the share keeps what moved into it, as a
            //    sync judges it where this device's own move puts it
            let in_share = |file: &Uuid| -> LbResult<bool> {
                Ok(file == id
                    || match synced.maybe_find(file) {
                        Some(_) => synced.ancestors(file)?.contains(id),
                        None => self.ancestors(file)?.contains(id),
                    })
            };
            let kept_in_share = |file: &Uuid| -> LbResult<bool> {
                Ok(in_share(file)? && (file == id || self.ancestors(file)?.contains(id)))
            };
            let mut undone = Vec::new();
            for &(moved, to, from) in &moves {
                if kept_in_share(&to)? && !in_share(&from)? {
                    tree.move_unvalidated(&moved, &from, keychain)?;
                    undone.push(moved);
                }
            }

            // 3. undoing can leave a folder inside itself. e.g. the server has /p/a/; here a was
            //    moved into the share, then p into a; putting a back under p makes p its own
            //    ancestor. so any recorded move whose file is now its own ancestor goes back too
            while let Some(&(moved, _, from)) = moves.iter().find(|(moved, _, _)| {
                !undone.contains(moved) && tree.ancestors(moved).is_ok_and(|a| a.contains(moved))
            }) {
                tree.move_unvalidated(&moved, &from, keychain)?;
                undone.push(moved);
            }

            // 4. each file put back takes the first free numbered name in its parent ("notes",
            //    then "notes 2"), in id order, so which of two same-named files keeps the name
            //    doesn't depend on the order the moves were found in
            undone.sort();
            for moved in undone {
                let parent = *tree.find(&moved)?.parent();
                let name = tree.name(&moved, keychain)?;
                let components = if tree.find(&moved)?.is_folder() {
                    NameComponents::folder(&name)
                } else {
                    NameComponents::from(&name)
                };
                let free = tree.free_name(&parent, &components, Some(moved), keychain)?;
                if free != name {
                    tree.rename_unvalidated(&moved, &free, keychain)?;
                }
            }
        }
        // the moves put back, their renames and the unshare are staged together, then validated
        // and promoted as one change
        let unshared = tree.delete_share_op(id, maybe_encrypted_for, keychain)?;
        tree.stage_and_promote(unshared)?;
        let (_, op) = tree.unstage();
        self.stage_validate_and_promote(op, me)?;
        Ok(())
    }

    pub fn update_document_unvalidated(
        &mut self, id: &Uuid, document: &[u8], keychain: &Keychain,
    ) -> LbResult<EncryptedDocument> {
        let (op, document) = self.update_document_op(id, document, keychain)?;
        self.stage_and_promote(Some(op))?;
        Ok(document)
    }

    pub fn overwrite_document_hmac_unvalidated(
        &mut self, id: &Uuid, new_hmac: Option<DocumentHmac>, new_size: Option<usize>,
        keychain: &Keychain,
    ) -> LbResult<()> {
        let op = self.overwrite_document_hmac_op(id, new_hmac, new_size, keychain)?;
        self.stage_and_promote(Some(op))?;
        Ok(())
    }

    pub fn update_document(
        &mut self, id: &Uuid, document: &[u8], keychain: &Keychain,
    ) -> LbResult<EncryptedDocument> {
        let (op, document) = self.update_document_op(id, document, keychain)?;
        self.stage_validate_and_promote(Some(op), Owner(keychain.get_pk()?))?;
        Ok(document)
    }

    pub fn overwrite_document_hmac(
        &mut self, id: &Uuid, new_hmac: Option<DocumentHmac>, new_size: Option<usize>,
        keychain: &Keychain,
    ) -> LbResult<()> {
        let op = self.overwrite_document_hmac_op(id, new_hmac, new_size, keychain)?;
        self.stage_validate_and_promote(Some(op), Owner(keychain.get_pk()?))?;
        Ok(())
    }
}
