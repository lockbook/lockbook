use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    access_info::{EncryptedFolderAccessKey, UserAccessInfo},
    file_metadata::{DocumentHmac, FileType, Owner},
    secret_filename::SecretFileName,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Meta {
    V1 {
        id: Uuid,
        file_type: FileType,
        parent: Uuid,
        name: SecretFileName,
        owner: Owner,
        is_deleted: bool,
        document_hmac: Option<DocumentHmac>,
        user_access_keys: Vec<UserAccessInfo>,
        folder_access_key: EncryptedFolderAccessKey,
    },
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
                    document_hmac,
                    user_access_keys,
                    // todo: verify that ignoring this is intentional
                    folder_access_key: _,
                },
                Meta::V1 {
                    id: other_id,
                    file_type: other_file_type,
                    parent: other_parent,
                    name: other_name,
                    owner: other_owner,
                    is_deleted: other_is_deleted,
                    document_hmac: other_document_hmac,
                    user_access_keys: other_user_access_keys,
                    // todo: verify that ignoring this is intentional
                    folder_access_key: _other_folder_access_key,
                },
            ) => {
                id == other_id
                    && file_type == other_file_type
                    && parent == other_parent
                    && name == other_name
                    && owner == other_owner
                    && is_deleted == other_is_deleted
                    && document_hmac == other_document_hmac
                    && user_access_keys == other_user_access_keys
            }
        }
    }
}
