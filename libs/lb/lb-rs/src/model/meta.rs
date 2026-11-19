use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use std::mem;
use uuid::Uuid;

use crate::LbResult;
use crate::service::keychain::Keychain;

use super::access_info::{EncryptedKey, UserAccessInfo, UserAccessMode};
use super::account::Account;
use super::clock::get_time;
use super::crypto::AESKey;
use super::entropy::Entropy;
use super::file_like::FileLike;
use super::file_metadata::{DocumentHmac, FileType, Owner};
use super::secret_filename::SecretFileName;
use super::signed_meta::SignedMeta;
use super::{pubkey, symkey};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Meta {
    V1 {
        id: Uuid,
        file_type: FileType,
        parent: Uuid,
        name: SecretFileName,
        owner: Owner,
        is_deleted: bool,
        doc_size: Option<usize>,
        doc_hmac: Option<DocumentHmac>,
        user_access_keys: Vec<UserAccessInfo>,
        folder_access_key: EncryptedKey,
    },
    /// Adds optional attachment info that marks this file as an attachment of another. A single
    /// attachment folder per note is created when the first image is added or the note is
    /// write-shared for the first time. This folder is moved, deleted, and shared alongside the
    /// target note. Links resolve into it under relaxed conditions. This folder is hidden from
    /// files, recents, shared with me, and search, but shown by `lockbook list` and files
    /// integrations.
    V2 {
        id: Uuid,
        file_type: FileType,
        parent: Uuid,
        name: SecretFileName,
        owner: Owner,
        is_deleted: bool,
        doc_size: Option<usize>,
        doc_hmac: Option<DocumentHmac>,
        user_access_keys: Vec<UserAccessInfo>,
        folder_access_key: EncryptedKey,
        attachment_of: Option<AttachmentAccessInfo>,
    },
}

/// The information required for this folder to follow the target note's lifecycle. The server
/// considers it part of the same tree as the target, so it hands it out to the same users from
/// `/get-updates`, and its key is encrypted with that of the target so it is readable to anyone
/// who can read the target.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AttachmentAccessInfo {
    pub id: Uuid,
    pub key: EncryptedKey,
}

impl Meta {
    pub fn create_root(account: &Account, entropy: &Entropy) -> LbResult<Self> {
        let id = entropy.id();
        let key = entropy.key();
        let pub_key = account.public_key();

        Ok(Meta::V1 {
            id,
            file_type: FileType::Folder,
            parent: id,
            name: SecretFileName::from_str(&account.username, &key, &key)?,
            owner: Owner(pub_key),
            is_deleted: false,
            doc_hmac: None,
            doc_size: None,
            user_access_keys: vec![UserAccessInfo::encrypt(
                account,
                &pub_key,
                &pub_key,
                &key,
                UserAccessMode::Write,
            )?],
            folder_access_key: symkey::encrypt(&key, &key)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create(
        id: Uuid, key: AESKey, owner: &PublicKey, parent: Uuid, parent_key: &AESKey, name: &str,
        file_type: FileType, attachment_of: Option<AttachmentAccessInfo>,
    ) -> LbResult<Self> {
        let name = SecretFileName::from_str(name, &key, parent_key)?;
        let owner = Owner(*owner);
        let folder_access_key = symkey::encrypt(parent_key, &key)?;
        Ok(match attachment_of {
            None => Meta::V1 {
                id,
                file_type,
                parent,
                name,
                owner,
                is_deleted: false,
                doc_hmac: None,
                doc_size: None,
                user_access_keys: Default::default(),
                folder_access_key,
            },
            Some(_) => Meta::V2 {
                id,
                file_type,
                parent,
                name,
                owner,
                is_deleted: false,
                doc_hmac: None,
                doc_size: None,
                user_access_keys: Default::default(),
                folder_access_key,
                attachment_of,
            },
        })
    }

    pub fn set_parent(&mut self, new_parent: Uuid) {
        match self {
            Meta::V1 { parent, .. } | Meta::V2 { parent, .. } => *parent = new_parent,
        }
    }

    pub fn set_name(&mut self, new_name: SecretFileName) {
        match self {
            Meta::V1 { name, .. } | Meta::V2 { name, .. } => *name = new_name,
        }
    }

    pub fn set_owner(&mut self, new_owner: Owner) {
        match self {
            Meta::V1 { owner, .. } | Meta::V2 { owner, .. } => *owner = new_owner,
        }
    }

    pub fn set_deleted(&mut self, new_is_deleted: bool) {
        match self {
            Meta::V1 { is_deleted, .. } | Meta::V2 { is_deleted, .. } => {
                *is_deleted = new_is_deleted
            }
        }
    }

    pub fn set_folder_access_keys(&mut self, new_keys: EncryptedKey) {
        match self {
            Meta::V1 { folder_access_key, .. } | Meta::V2 { folder_access_key, .. } => {
                *folder_access_key = new_keys
            }
        }
    }

    pub fn user_access_keys_mut(&mut self) -> &mut Vec<UserAccessInfo> {
        match self {
            Meta::V1 { user_access_keys, .. } | Meta::V2 { user_access_keys, .. } => {
                user_access_keys
            }
        }
    }

    pub fn set_hmac_and_size(&mut self, new_hmac: Option<DocumentHmac>, new_size: Option<usize>) {
        match self {
            Meta::V1 { doc_hmac, doc_size, .. } | Meta::V2 { doc_hmac, doc_size, .. } => {
                *doc_hmac = new_hmac;
                *doc_size = new_size;
            }
        }
    }

    pub fn set_type(&mut self, new_type: FileType) {
        match self {
            Meta::V1 { file_type, .. } | Meta::V2 { file_type, .. } => *file_type = new_type,
        }
    }

    pub fn set_id(&mut self, new_id: Uuid) {
        match self {
            Meta::V1 { id, .. } | Meta::V2 { id, .. } => *id = new_id,
        }
    }

    // todo move to FileLike when possible
    pub fn doc_size(&self) -> &Option<usize> {
        match self {
            Meta::V1 { doc_size, .. } | Meta::V2 { doc_size, .. } => doc_size,
        }
    }

    pub fn sign(self, keychain: &Keychain) -> LbResult<SignedMeta> {
        pubkey::sign(&keychain.get_account()?.private_key, &keychain.get_pk()?, self, get_time)
    }

    pub fn sign_with(self, account: &Account) -> LbResult<SignedMeta> {
        pubkey::sign(&account.private_key, &account.public_key(), self, get_time)
    }
}

// This is impl'd to avoid comparing encrypted values
impl PartialEq for Meta {
    fn eq(&self, other: &Self) -> bool {
        // todo: verify that ignoring folder_access_key is intentional
        mem::discriminant(self) == mem::discriminant(other)
            && self.id() == other.id()
            && self.file_type() == other.file_type()
            && self.parent() == other.parent()
            && self.secret_name() == other.secret_name()
            && self.owner() == other.owner()
            && self.explicitly_deleted() == other.explicitly_deleted()
            && self.document_hmac() == other.document_hmac()
            && self.user_access_keys() == other.user_access_keys()
            && self.doc_size() == other.doc_size()
            && self.attachment_of() == other.attachment_of()
    }
}
