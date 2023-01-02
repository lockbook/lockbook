use crate::billing::billing_model::SubscriptionProfile;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::server_file::ServerFile;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub billing_info: SubscriptionProfile,
}

hmdb::schema! {
    ServerV1 {
        usernames: <String, Owner>,
        accounts: <Owner, Account>,
        owned_files: <Owner, HashSet<Uuid>>,
        metas: <Uuid, ServerFile>,
        sizes: <Uuid, u64>,
        google_play_ids: <String, Owner>,
        stripe_ids: <String, Owner>
    }
}

pub mod v2 {
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
            stripe_ids: <String, Owner>
        }
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
            last_seen: <Owner, u64>
        }
    }

    pub fn migrate(
        source: &mut super::v2::transaction::Server,
        destination: &mut super::v3::transaction::Server,
    ) {
        // copy existing tables
        for (k, v) in source.usernames.get_all() {
            destination.usernames.insert(k.clone(), *v);
        }
        for (k, v) in source.accounts.get_all() {
            destination.accounts.insert(*k, v.clone());
        }
        for (k, v) in source.metas.get_all() {
            destination.metas.insert(*k, v.clone());
        }
        for (k, v) in source.sizes.get_all() {
            destination.sizes.insert(*k, *v);
        }
        for (k, v) in source.google_play_ids.get_all() {
            destination.google_play_ids.insert(k.clone(), *v);
        }
        for (k, v) in source.stripe_ids.get_all() {
            destination.stripe_ids.insert(k.clone(), *v);
        }
        for (k, v) in source.owned_files.get_all() {
            destination.owned_files.insert(*k, v.clone());
        }
        for (k, v) in source.shared_files.get_all() {
            destination.shared_files.insert(*k, v.clone());
        }
        for (k, v) in source.file_children.get_all() {
            destination.file_children.insert(*k, v.clone());
        }
    }
}
