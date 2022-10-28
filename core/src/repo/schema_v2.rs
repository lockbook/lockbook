use crate::repo::schema::CoreV1;
use crate::CoreError;
use hmdb::transaction::Transaction;
use lockbook_shared::account::Account;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::{document_repo, SharedError};
use serde::{Deserialize, Serialize};
use std::fs;
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
    pub fn init_with_migration(writeable_path: &str) -> Result<CoreV2, CoreError> {
        let db = CoreV2::init(writeable_path)?;

        // migrate metadata from v1
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

                Ok::<_, hmdb::errors::Error>(())
            })
            .expect("failed to migrate local database from v1 to v2")?;
        }

        // migrate documents from id+source structure to id+hmac structure
        let base_path = format!("{}/all_base_documents", writeable_path);
        let base_path = Path::new(&base_path);
        let local_path = format!("{}/changed_local_documents", writeable_path);
        let local_path = Path::new(&local_path);
        let docs_path = document_repo::namespace_path(writeable_path);
        if base_path.is_dir() && local_path.is_dir() {
            // create docs directory
            println!("create_dir_all v");
            fs::create_dir_all(&docs_path)?;
            println!("^");

            // move/rename base files
            for entry in fs::read_dir(&base_path)? {
                let path = entry?.path();
                let id_str = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or(SharedError::Unexpected("document disk file name malformed"))?;
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| SharedError::Unexpected("document disk file name malformed"))?;
                let hmac = db
                    .base_metadata
                    .get(&id)?
                    .and_then(|f| f.document_hmac().cloned())
                    .ok_or(CoreError::Unexpected(format!(
                        "hmac in metadata missing for disk file {:?}",
                        id
                    )))?;
                println!("rename v");
                fs::rename(path, document_repo::key_path(writeable_path, &id, &hmac))?;
                println!("^");
            }

            // move/rename local files
            for entry in fs::read_dir(&local_path)? {
                let path = entry?.path();
                let id_str = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or(SharedError::Unexpected("document disk file name malformed"))?;
                let id = Uuid::parse_str(id_str)
                    .map_err(|_| SharedError::Unexpected("document disk file name malformed"))?;
                let hmac = db
                    .local_metadata
                    .get(&id)?
                    .and_then(|f| f.document_hmac().cloned())
                    .ok_or(SharedError::Unexpected("hmac in metadata missing for disk file"))?;
                fs::rename(path, document_repo::key_path(writeable_path, &id, &hmac))?;
            }

            // remove emptied directories
            // fs::remove_dir_all(base_path)?;
            // fs::remove_dir_all(local_path)?;
        }

        Ok(db)
    }
}
