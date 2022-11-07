use crate::billing::billing_model::SubscriptionProfile;
use lockbook_shared::file::metadata::Owner;
use lockbook_shared::file::server::ServerFile;
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
    use lockbook_shared::file::like::FileLike;
    use lockbook_shared::file::metadata::Owner;
    use lockbook_shared::file::server::ServerFile;
    use std::collections::HashSet;
    use tracing::info;
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

    pub fn migrate(
        source: &mut super::transaction::ServerV1, destination: &mut transaction::Server,
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
            if source.metas.get(k).is_some() {
                destination.sizes.insert(*k, *v);
            }
        }
        for (k, v) in source.google_play_ids.get_all() {
            destination.google_play_ids.insert(k.clone(), *v);
        }
        for (k, v) in source.stripe_ids.get_all() {
            destination.stripe_ids.insert(k.clone(), *v);
        }

        // populate new indexes (and regenerate owned files, which is corrupt)
        let mut owned_files = HashMap::new();
        let mut shared_files = HashMap::new();
        let mut file_children = HashMap::new();
        for owner in source.accounts.keys() {
            owned_files.insert(*owner, HashSet::new());
            shared_files.insert(*owner, HashSet::new());
        }
        for (id, file) in source.metas.get_all() {
            file_children.insert(*id, HashSet::new());
            if let Some(owned_files) = owned_files.get_mut(&file.owner()) {
                owned_files.insert(*id);
            }
            for user_access_key in file.user_access_keys() {
                if user_access_key.encrypted_for != user_access_key.encrypted_by {
                    if let Some(shared_files) =
                        shared_files.get_mut(&Owner(user_access_key.encrypted_for))
                    {
                        shared_files.insert(*id);
                    }
                }
            }
        }
        for (id, file) in source.metas.get_all() {
            if let Some(file_children) = file_children.get_mut(file.parent()) {
                file_children.insert(*id);
            }
        }
        {
            let num_users = owned_files.len();
            info!(?num_users, "migration: indexed owned files");
        }
        for (k, v) in owned_files {
            destination.owned_files.insert(k, v);
        }
        {
            let num_users = shared_files.len();
            info!(?num_users, "migration: indexed shared files");
        }
        for (k, v) in shared_files {
            destination.shared_files.insert(k, v);
        }
        {
            let num_files = file_children.len();
            info!(?num_files, "migration: indexed children of files");
        }
        for (k, v) in file_children {
            destination.file_children.insert(k, v);
        }
    }
}
