use lockbook_models::account::Account;
use lockbook_models::file_metadata::EncryptedFileMetadata;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

hmdb::schema! {
    CoreV1 {
        account: <OneKey, Account>,
        last_synced: <OneKey, i64>,
        root: <OneKey, Uuid>,
        local_digest: <Uuid, Vec<u8>>,
        base_digest: <Uuid, Vec<u8>>,
        local_metadata: <Uuid, EncryptedFileMetadata>,
        base_metadata: <Uuid, EncryptedFileMetadata>
    }
}
