use crate::billing::billing_model::SubscriptionProfile;
use crate::schema::v3::transaction;
use db_rs::{Db, LookupSet, LookupTable};
use db_rs_derive::Schema;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::server_file::ServerFile;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub billing_info: SubscriptionProfile,
}

pub type ServerDb = ServerV4;

#[derive(Schema)]
pub struct ServerV4 {
    pub usernames: LookupTable<String, Owner>,
    pub metas: LookupTable<Uuid, ServerFile>,
    pub sizes: LookupTable<Uuid, u64>,
    pub google_play_ids: LookupTable<String, Owner>,
    pub stripe_ids: LookupTable<String, Owner>,
    pub app_store_ids: LookupTable<String, Owner>,
    pub last_seen: LookupTable<Owner, u64>,
    pub accounts: LookupTable<Owner, Account>,
    pub owned_files: LookupSet<Owner, Uuid>,
    pub shared_files: LookupSet<Owner, Uuid>,
    pub file_children: LookupSet<Uuid, Uuid>,
}

impl ServerV4 {
    pub fn migrate(src: &transaction::Server, dest: &mut ServerV4) {
        let tx = dest.begin_transaction().unwrap();

        for (k, v) in src.usernames.get_all().clone() {
            dest.usernames.insert(k, v).unwrap();
        }

        for (k, v) in src.metas.get_all().clone() {
            dest.metas.insert(k, v).unwrap();
        }

        for (k, v) in src.sizes.get_all().clone() {
            dest.sizes.insert(k, v).unwrap();
        }

        for (k, v) in src.google_play_ids.get_all().clone() {
            dest.google_play_ids.insert(k, v).unwrap();
        }

        for (k, v) in src.stripe_ids.get_all().clone() {
            dest.stripe_ids.insert(k, v).unwrap();
        }

        for (k, v) in src.app_store_ids.get_all().clone() {
            dest.app_store_ids.insert(k, v).unwrap();
        }

        for (k, v) in src.last_seen.get_all().clone() {
            dest.last_seen.insert(k, v).unwrap();
        }

        for (k, v) in src.accounts.get_all().clone() {
            dest.accounts.insert(k, v).unwrap();
        }

        for (owner, uuids) in src.owned_files.get_all().clone() {
            for uuid in uuids {
                dest.owned_files.insert(owner, uuid).unwrap();
            }
        }

        for (owner, uuids) in src.shared_files.get_all().clone() {
            for uuid in uuids {
                dest.shared_files.insert(owner, uuid).unwrap();
            }
        }

        for (parent, children) in src.file_children.get_all().clone() {
            for child in children {
                dest.file_children.insert(parent, child).unwrap();
            }
        }

        tx.drop_safely().unwrap();
    }
}

pub mod v3 {
    use super::Account;
    use lockbook_shared::file_metadata::Owner;
    use lockbook_shared::server_file::ServerFile;
    use std::collections::HashSet;
    use uuid::Uuid;

    hmdb::schema! {
        Server {
            usernames: <String, Owner>,
            accounts: <Owner, Account>,
            owned_files: <Owner, HashSet<Uuid>>,
            shared_files: <Owner, HashSet<Uuid>>,
            metas: <Uuid, ServerFile>,
            file_children: <Uuid, HashSet<Uuid>>,
            sizes: <Uuid, u64>,
            google_play_ids: <String, Owner>,
            stripe_ids: <String, Owner>,
            app_store_ids: <String, Owner>,
            last_seen: <Owner, u64>
        }
    }
}
