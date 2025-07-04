use std::sync::Arc;

use crate::billing::billing_model::SubscriptionProfile;
use db_rs::{LookupSet, LookupTable};
use db_rs_derive::Schema;
use futures::lock::Mutex;
use lb_rs::model::meta::Meta;
use lb_rs::model::server_file::ServerFile;
use lb_rs::model::{file_metadata::Owner, server_meta::ServerMeta};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub billing_info: SubscriptionProfile,
}

pub type ServerDb = ServerV5;

#[derive(Schema)]
#[cfg_attr(feature = "no-network", derive(Clone))]
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

#[derive(Schema)]
#[cfg_attr(feature = "no-network", derive(Clone))]
pub struct ServerV5 {
    pub usernames: LookupTable<String, Owner>,
    pub metas: LookupTable<Uuid, ServerMeta>,
    pub google_play_ids: LookupTable<String, Owner>,
    pub stripe_ids: LookupTable<String, Owner>,
    pub app_store_ids: LookupTable<String, Owner>,
    pub last_seen: LookupTable<Owner, u64>,
    pub accounts: LookupTable<Owner, Account>,
    pub owned_files: LookupSet<Owner, Uuid>,
    pub shared_files: LookupSet<Owner, Uuid>,
    pub file_children: LookupSet<Uuid, Uuid>,
}

async fn migrate(v4: Arc<Mutex<ServerV4>>, v5: Arc<RwLock<ServerV5>>) {
    let v4 = v4.lock().await;
    let mut v5 = v5.write().await;

    for (user, owner) in v4.usernames.get() {
        v5.usernames.insert(user.clone(), *owner).unwrap();
    }

    for (id, meta) in v4.metas.get() {
        v5.metas
            .insert(*id, ServerMeta::from(meta.clone()))
            .unwrap();
    }

    for (id, size) in v4.sizes.get() {
        let mut meta = v5.metas.remove(id).unwrap().unwrap();
        match &mut meta.file.timestamped_value.value {
            Meta::V1 { doc_size, .. } => {
                *doc_size = Some(*size as usize);
            }
        }

        v5.metas.insert(*id, meta).unwrap();
    }

    for (gpid, owner) in v4.google_play_ids.get() {
        v5.google_play_ids.insert(gpid.clone(), *owner).unwrap();
    }

    for (stripe_id, owner) in v4.stripe_ids.get() {
        v5.stripe_ids.insert(stripe_id.clone(), *owner).unwrap();
    }

    for (app_store_id, owner) in v4.app_store_ids.get() {
        v5.app_store_ids
            .insert(app_store_id.clone(), *owner)
            .unwrap();
    }

    for (owner, last_seen) in v4.last_seen.get() {
        v5.last_seen.insert(*owner, *last_seen).unwrap();
    }

    for (owner, account) in v4.accounts.get() {
        v5.accounts.insert(*owner, account.clone()).unwrap();
    }

    for (owner, owned_files) in v4.owned_files.get() {
        for owned_file in owned_files {
            v5.owned_files.insert(*owner, owned_file.clone()).unwrap();
        }
    }

    for (owner, shared_files) in v4.shared_files.get() {
        for shared_file in shared_files {
            v5.shared_files.insert(*owner, *shared_file).unwrap();
        }
    }

    for (id, children) in v4.file_children.get() {
        for child in children {
            v5.file_children.insert(*id, *child).unwrap();
        }
    }
}
