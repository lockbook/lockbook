use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::access_info::{EncryptedFolderAccessKey, UserAccessInfo};
use super::file_metadata::{DocumentHmac, FileType, Owner};
use super::secret_filename::SecretFileName;

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
    pub fn doc_size(&self) -> &Option<usize> {
        match self {
            Meta::V1 { doc_size, .. } => doc_size,
        }
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
