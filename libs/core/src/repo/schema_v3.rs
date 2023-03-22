use hmdb::transaction::Transaction;
use lockbook_shared::account::Account;
use lockbook_shared::document_repo::DocEvents;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::signed_file::SignedFile;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::schema_v2::CoreV2;

pub type Tx<'a> = transaction::CoreV3<'a>;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

hmdb::schema! {
    CoreV3 {
        account: <OneKey, Account>,
        last_synced: <OneKey, i64>,
        root: <OneKey, Uuid>,
        local_metadata: <Uuid, SignedFile>,
        base_metadata: <Uuid, SignedFile>,
        public_key_by_username: <String, Owner>,
        username_by_public_key: <Owner, String>,
        docs_events: <Uuid, Vec<DocEvents>>
    }
}

impl CoreV3 {
    pub fn init_with_migration(writeable_path: &str) -> Result<CoreV3, CoreError> {
        let db = CoreV3::init(writeable_path)?;

        if db.account.get_all()?.is_empty() {
            let source = CoreV2::init(writeable_path)?;
            let result = db.transaction(|tx| {
                for v in source.account.get_all()?.into_values() {
                    tx.account.insert(OneKey {}, v);
                }
                for v in source.last_synced.get_all()?.into_values() {
                    tx.last_synced.insert(OneKey {}, v);
                }
                for v in source.root.get_all()?.into_values() {
                    tx.root.insert(OneKey {}, v);
                }
                for (k, v) in source.local_metadata.get_all()? {
                    tx.local_metadata.insert(k, v);
                }
                for (k, v) in source.base_metadata.get_all()? {
                    tx.base_metadata.insert(k, v);
                }
                for (k, v) in source.public_key_by_username.get_all()? {
                    tx.public_key_by_username.insert(k, v);
                }
                for (k, v) in source.username_by_public_key.get_all()? {
                    tx.username_by_public_key.insert(k, v);
                }
                Ok::<_, hmdb::errors::Error>(())
            });
            if result.is_err() {
                Err(CoreError::Unexpected(
                    "failed to migrate local database from v2 to v3".to_string(),
                ))?;
            }
        }

        Ok(db)
    }
}
