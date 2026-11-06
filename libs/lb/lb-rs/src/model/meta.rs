use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::LbResult;
use crate::service::keychain::Keychain;

use super::access_info::{EncryptedFolderAccessKey, UserAccessInfo, UserAccessMode};
use super::account::Account;
use super::clock::get_time;
use super::crypto::AESKey;
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
        folder_access_key: EncryptedFolderAccessKey,
    },
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
        folder_access_key: EncryptedFolderAccessKey,
        /// On a folder: the document whose attachments it holds.
        attachments_of: Option<AttachmentsOf>,
    },
    V3 {
        id: Uuid,
        file_type: FileType,
        parent: Uuid,
        name: SecretFileName,
        owner: Owner,
        is_deleted: bool,
        user_access_keys: Vec<UserAccessInfo>,
        folder_access_key: EncryptedFolderAccessKey,
        attachments_of: Option<AttachmentsOf>,
    },
}

/// A document's id, and the folder's key encrypted with that document's key.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AttachmentsOf {
    pub id: Uuid,
    pub key: EncryptedFolderAccessKey,
}

impl Meta {
    pub fn create_root(account: &Account) -> LbResult<Self> {
        let id = Uuid::new_v4();
        let key = symkey::generate_key();
        let pub_key = account.public_key();

        Ok(Meta::V3 {
            id,
            file_type: FileType::Folder,
            parent: id,
            name: SecretFileName::from_str(&account.username, &key, &key)?,
            owner: Owner(pub_key),
            is_deleted: false,
            user_access_keys: vec![UserAccessInfo::encrypt(
                account,
                &pub_key,
                &pub_key,
                &key,
                UserAccessMode::Write,
            )?],
            folder_access_key: symkey::encrypt(&key, &key)?,
            attachments_of: None,
        })
    }

    pub fn create(
        id: Uuid, key: AESKey, owner: &PublicKey, parent: Uuid, parent_key: &AESKey, name: &str,
        file_type: FileType,
    ) -> LbResult<Self> {
        Ok(Meta::V3 {
            id,
            file_type,
            parent,
            name: SecretFileName::from_str(name, &key, parent_key)?,
            owner: Owner(*owner),
            is_deleted: false,
            user_access_keys: Default::default(),
            folder_access_key: symkey::encrypt(parent_key, &key)?,
            attachments_of: None,
        })
    }

    pub fn is_v3(&self) -> bool {
        matches!(self, Meta::V3 { .. })
    }

    pub fn into_v3(mut self) -> Self {
        self.upgrade();
        self
    }

    pub fn into_v1(self, doc_hmac: Option<DocumentHmac>, doc_size: Option<usize>) -> Self {
        match self.into_v3() {
            Meta::V3 {
                id,
                file_type,
                parent,
                name,
                owner,
                is_deleted,
                user_access_keys,
                folder_access_key,
                ..
            } => Meta::V1 {
                id,
                file_type,
                parent,
                name,
                owner,
                is_deleted,
                doc_size,
                doc_hmac,
                user_access_keys,
                folder_access_key,
            },
            Meta::V1 { .. } | Meta::V2 { .. } => unreachable!(),
        }
    }

    pub fn same_file_state(&self, other: &Self) -> bool {
        self.clone().into_v3() == other.clone().into_v3()
    }

    pub fn set_parent(&mut self, new_parent: Uuid) {
        match self {
            Meta::V1 { parent, .. } | Meta::V2 { parent, .. } | Meta::V3 { parent, .. } => {
                *parent = new_parent
            }
        }
    }

    pub fn set_name(&mut self, new_name: SecretFileName) {
        match self {
            Meta::V1 { name, .. } | Meta::V2 { name, .. } | Meta::V3 { name, .. } => {
                *name = new_name
            }
        }
    }

    pub fn set_owner(&mut self, new_owner: Owner) {
        match self {
            Meta::V1 { owner, .. } | Meta::V2 { owner, .. } | Meta::V3 { owner, .. } => {
                *owner = new_owner
            }
        }
    }

    pub fn set_deleted(&mut self, new_is_deleted: bool) {
        match self {
            Meta::V1 { is_deleted, .. }
            | Meta::V2 { is_deleted, .. }
            | Meta::V3 { is_deleted, .. } => *is_deleted = new_is_deleted,
        }
    }

    pub fn set_folder_access_keys(&mut self, new_keys: EncryptedFolderAccessKey) {
        match self {
            Meta::V1 { folder_access_key, .. }
            | Meta::V2 { folder_access_key, .. }
            | Meta::V3 { folder_access_key, .. } => *folder_access_key = new_keys,
        }
    }

    pub fn user_access_keys_mut(&mut self) -> &mut Vec<UserAccessInfo> {
        match self {
            Meta::V1 { user_access_keys, .. }
            | Meta::V2 { user_access_keys, .. }
            | Meta::V3 { user_access_keys, .. } => user_access_keys,
        }
    }

    pub fn set_type(&mut self, new_type: FileType) {
        match self {
            Meta::V1 { file_type, .. }
            | Meta::V2 { file_type, .. }
            | Meta::V3 { file_type, .. } => *file_type = new_type,
        }
    }

    pub fn set_id(&mut self, new_id: Uuid) {
        match self {
            Meta::V1 { id, .. } | Meta::V2 { id, .. } | Meta::V3 { id, .. } => *id = new_id,
        }
    }

    // todo move to FileLike when possible
    pub fn doc_size(&self) -> &Option<usize> {
        match self {
            Meta::V1 { doc_size, .. } | Meta::V2 { doc_size, .. } => doc_size,
            Meta::V3 { .. } => &None,
        }
    }

    pub fn set_attachments_of(&mut self, new_attachments_of: Option<AttachmentsOf>) {
        let Meta::V3 { attachments_of, .. } = self.upgrade() else { unreachable!() };
        *attachments_of = new_attachments_of;
    }

    fn upgrade(&mut self) -> &mut Self {
        let upgraded = match self.clone() {
            Meta::V1 {
                id,
                file_type,
                parent,
                name,
                owner,
                is_deleted,
                user_access_keys,
                folder_access_key,
                ..
            } => Some(Meta::V3 {
                id,
                file_type,
                parent,
                name,
                owner,
                is_deleted,
                user_access_keys,
                folder_access_key,
                attachments_of: None,
            }),
            Meta::V2 {
                id,
                file_type,
                parent,
                name,
                owner,
                is_deleted,
                user_access_keys,
                folder_access_key,
                attachments_of,
                ..
            } => Some(Meta::V3 {
                id,
                file_type,
                parent,
                name,
                owner,
                is_deleted,
                user_access_keys,
                folder_access_key,
                attachments_of,
            }),
            Meta::V3 { .. } => None,
        };
        if let Some(upgraded) = upgraded {
            *self = upgraded;
        }
        self
    }

    // every change a client signs is written as V3
    pub fn sign(mut self, keychain: &Keychain) -> LbResult<SignedMeta> {
        self.upgrade();
        pubkey::sign(&keychain.get_account()?.private_key, &keychain.get_pk()?, self, get_time)
    }

    pub fn sign_with(mut self, account: &Account) -> LbResult<SignedMeta> {
        self.upgrade();
        pubkey::sign(&account.private_key, &account.public_key(), self, get_time)
    }

    pub fn sign_as_is(self, account: &Account) -> LbResult<SignedMeta> {
        pubkey::sign(&account.private_key, &account.public_key(), self, get_time)
    }
}

// Skips values re-encrypted without changing, but compares attachments_of whole so a
// change to it is never taken for no change. A V1 equals a V2 without attachments_of.
impl PartialEq for Meta {
    fn eq(&self, other: &Self) -> bool {
        // todo: verify that ignoring folder_access_key is intentional
        self.id() == other.id()
            && self.file_type() == other.file_type()
            && self.parent() == other.parent()
            && self.secret_name() == other.secret_name()
            && self.owner() == other.owner()
            && self.explicitly_deleted() == other.explicitly_deleted()
            && self.document_hmac() == other.document_hmac()
            && self.user_access_keys() == other.user_access_keys()
            && FileLike::doc_size(self) == FileLike::doc_size(other)
            && self.attachments_of() == other.attachments_of()
            && self.is_v3() == other.is_v3()
    }
}
