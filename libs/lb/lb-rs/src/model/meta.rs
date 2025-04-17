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
