use crate::repo::schema::CoreV1;
use hmdb::transaction::Transaction;
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
    pub fn init_with_migration(writeable_path: &str) -> Result<CoreV2, hmdb::errors::Error> {
        let db = CoreV2::init(writeable_path)?;
        if db.account.get_all()?.is_empty() {
            let source = CoreV1::init(writeable_path)?;
            db.transaction(|tx| {
                // copy existing tables (new tables populated on next sync)
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

                Ok(())
            })
            .expect("failed to migrate local database from v1 to v2")?;
        }
        Ok(db)
    }
}
