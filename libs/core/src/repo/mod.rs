use db_rs::{Db, LookupTable, Single};
use db_rs_derive::Schema;
use hmdb::log::Reader;
use std::fs::remove_file;
use std::path::PathBuf;

use lockbook_shared::account::Account;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::signed_file::SignedFile;

use crate::repo::schema_v2::{CoreV2, OneKey};
use crate::CoreResult;
use lockbook_shared::core_config::Config;
use uuid::Uuid;

pub mod schema;
pub mod schema_v2;

pub type CoreDb = CoreV3;

#[derive(Schema, Debug)]
pub struct CoreV3 {
    pub account: Single<Account>,
    pub last_synced: Single<i64>,
    pub root: Single<Uuid>,
    pub local_metadata: LookupTable<Uuid, SignedFile>,
    pub base_metadata: LookupTable<Uuid, SignedFile>,
    pub pub_key_lookup: LookupTable<Owner, String>,
}

impl CoreV3 {
    pub fn init_with_migration(config: &Config) -> CoreResult<CoreV3> {
        let mut dest = CoreV3::init(db_rs::Config::in_folder(&config.writeable_path))?;
        if dest.account.data().is_none() {
            let source = CoreV2::init(&config.writeable_path)?;
            if let Some(account) = source.account.get(&OneKey {})? {
                let tx = dest.begin_transaction()?;
                dest.account.insert(account)?;

                if let Some(last_synced) = source.last_synced.get(&OneKey {})? {
                    dest.last_synced.insert(last_synced)?;
                }

                if let Some(root) = source.root.get(&OneKey {})? {
                    dest.root.insert(root)?;
                }

                for (k, v) in source.local_metadata.get_all()? {
                    dest.local_metadata.insert(k, v)?;
                }

                for (k, v) in source.base_metadata.get_all()? {
                    dest.base_metadata.insert(k, v)?;
                }

                for (owner, username) in source.username_by_public_key.get_all()? {
                    dest.pub_key_lookup.insert(owner, username)?;
                }

                tx.drop_safely()?;
            }

            drop(source);

            let mut path = PathBuf::from(&config.writeable_path);
            path.push("lockbook_core__repo__schema_v2__CoreV2");
            // ignore if file is missing
            drop(remove_file(path));

            let mut path = PathBuf::from(&config.writeable_path);
            path.push("lockbook_core__repo__schema__CoreV1");
            // ignore if file is missing
            drop(remove_file(path));
        }

        Ok(dest)
    }
}
