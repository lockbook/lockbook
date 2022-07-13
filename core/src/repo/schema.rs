use lockbook_models::account::Account;
use lockbook_models::file_metadata::UnsignedFile;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Tx<'a> = transaction::CoreV1<'a>;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

hmdb::schema! {
    CoreV1 {
        account: <OneKey, Account>,
        last_synced: <OneKey, i64>,
        root: <OneKey, Uuid>,
        local_digest: <Uuid, Vec<u8>>,
        base_digest: <Uuid, Vec<u8>>,
        local_metadata: <Uuid, UnsignedFile>,
        base_metadata: <Uuid, UnsignedFile>
    }
}
