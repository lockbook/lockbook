use lockbook_shared::account::Account;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::signed_file::SignedFile;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Tx<'a> = transaction::CoreV2<'a>;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

hmdb::schema! {
    CoreV2 {
        account: <OneKey, Account>,
        last_synced: <OneKey, i64>,
        root: <OneKey, Uuid>,
        local_metadata: <Uuid, SignedFile>,
        base_metadata: <Uuid, SignedFile>,
        public_key_by_username: <String, Owner>,
        username_by_public_key: <Owner, String>
    }
}

impl CoreV2 {
    pub fn init_with_migration(&self, writeable_path: &str) -> Result<CoreV2, hmdb::errors::Error> {
        // todo: if this fails, try V1
        CoreV2::init(writeable_path)
    }
}
