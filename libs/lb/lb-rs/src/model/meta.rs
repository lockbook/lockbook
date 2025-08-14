use libsecp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::LbResult;
use crate::service::keychain::Keychain;

use super::access_info::{EncryptedFolderAccessKey, UserAccessInfo, UserAccessMode};
use super::account::Account;
use super::clock::get_time;
use super::crypto::AESKey;
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
}

impl Meta {
    pub fn create_root(account: &Account) -> LbResult<Self> {
        let id = Uuid::new_v4();
        let key = symkey::generate_key();
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

    pub fn create(
        id: Uuid, key: AESKey, owner: &PublicKey, parent: Uuid, parent_key: &AESKey, name: &str,
        file_type: FileType,
    ) -> LbResult<Self> {
        Ok(Meta::V1 {
            id,
            file_type,
            parent,
            name: SecretFileName::from_str(name, &key, parent_key)?,
            owner: Owner(*owner),
            is_deleted: false,
            doc_hmac: None,
            doc_size: None,
            user_access_keys: Default::default(),
            folder_access_key: symkey::encrypt(parent_key, &key)?,
        })
    }

    pub fn set_parent(&mut self, new_parent: Uuid) {
        match self {
            Meta::V1 { parent, .. } => *parent = new_parent,
        }
    }

    pub fn set_name(&mut self, new_name: SecretFileName) {
        match self {
            Meta::V1 { name, .. } => *name = new_name,
        }
    }

    pub fn set_owner(&mut self, new_owner: Owner) {
        match self {
            Meta::V1 { owner, .. } => *owner = new_owner,
        }
    }

    pub fn set_deleted(&mut self, new_is_deleted: bool) {
        match self {
            Meta::V1 { is_deleted, .. } => *is_deleted = new_is_deleted,
        }
    }

    pub fn set_folder_access_keys(&mut self, new_keys: EncryptedFolderAccessKey) {
        match self {
            Meta::V1 { folder_access_key, .. } => *folder_access_key = new_keys,
        }
    }

    pub fn user_access_keys_mut(&mut self) -> &mut Vec<UserAccessInfo> {
        match self {
            Meta::V1 { user_access_keys, .. } => user_access_keys,
        }
    }

    pub fn set_hmac(&mut self, new_hmac: Option<DocumentHmac>) {
        match self {
            Meta::V1 { doc_hmac, .. } => *doc_hmac = new_hmac,
        }
    }

    pub fn set_type(&mut self, new_type: FileType) {
        match self {
            Meta::V1 { file_type, .. } => *file_type = new_type,
        }
    }

    pub fn set_id(&mut self, new_id: Uuid) {
        match self {
            Meta::V1 { id, .. } => *id = new_id,
        }
    }

    pub fn doc_size(&self) -> &Option<usize> {
        match self {
            Meta::V1 { doc_size, .. } => doc_size,
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
        match (self, other) {
            (
                Meta::V1 {
                    id,
                    file_type,
                    parent,
                    name,
                    owner,
                    is_deleted,
                    doc_hmac,
                    user_access_keys,
                    // todo: verify that ignoring this is intentional
                    folder_access_key: _,
                    doc_size,
                },
                Meta::V1 {
                    id: other_id,
                    file_type: other_file_type,
                    parent: other_parent,
                    name: other_name,
                    owner: other_owner,
                    is_deleted: other_is_deleted,
                    doc_hmac: other_doc_hmac,
                    user_access_keys: other_user_access_keys,
                    // todo: verify that ignoring this is intentional
                    folder_access_key: _other_folder_access_key,
                    doc_size: other_doc_size,
                },
            ) => {
                id == other_id
                    && file_type == other_file_type
                    && parent == other_parent
                    && name == other_name
                    && owner == other_owner
                    && is_deleted == other_is_deleted
                    && doc_hmac == other_doc_hmac
                    && user_access_keys == other_user_access_keys
                    && doc_size == other_doc_size
            }
        }
    }
}
