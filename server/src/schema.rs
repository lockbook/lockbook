use std::sync::Arc;

use crate::billing::billing_model::SubscriptionProfile;
use db_rs::{LookupSet, LookupTable};
use db_rs_derive::Schema;
use lb_rs::model::meta::Meta;
use lb_rs::model::server_file::ServerFile;
use lb_rs::model::{file_metadata::Owner, server_meta::ServerMeta};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::*;
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

pub async fn migrate(v4: Arc<Mutex<ServerV4>>, v5: Arc<Mutex<ServerV5>>) {
    let v4 = v4.lock().await;
    let mut v5 = v5.lock().await;

    for (user, owner) in v4.usernames.get() {
        v5.usernames.insert(user.clone(), *owner).unwrap();
    }
    info!("migrated {} usernames", v5.usernames.get().len());

    for (id, meta) in v4.metas.get() {
        v5.metas
            .insert(*id, ServerMeta::from(meta.clone()))
            .unwrap();
    }
    info!("migrated {} metas", v5.metas.get().len());

    for (id, size) in v4.sizes.get() {
        if let Some(mut meta) = v5.metas.remove(id).unwrap() {
            match &mut meta.file.timestamped_value.value {
                Meta::V1 { doc_size, .. } => {
                    *doc_size = Some(*size as usize);
                }
            }

            v5.metas.insert(*id, meta).unwrap();
        } else {
            warn!("{id} has size but no meta");
        }
    }

    for (gpid, owner) in v4.google_play_ids.get() {
        v5.google_play_ids.insert(gpid.clone(), *owner).unwrap();
    }
    info!("migrated {} gpids", v5.google_play_ids.get().len());

    for (stripe_id, owner) in v4.stripe_ids.get() {
        v5.stripe_ids.insert(stripe_id.clone(), *owner).unwrap();
    }
    info!("migrated {} stripe_ids", v5.stripe_ids.get().len());

    for (app_store_id, owner) in v4.app_store_ids.get() {
        v5.app_store_ids
            .insert(app_store_id.clone(), *owner)
            .unwrap();
    }
    info!("migrated {} apsids", v5.app_store_ids.get().len());

    for (owner, last_seen) in v4.last_seen.get() {
        v5.last_seen.insert(*owner, *last_seen).unwrap();
    }
    info!("migrated {} last_seens", v5.last_seen.get().len());

    for (owner, account) in v4.accounts.get() {
        v5.accounts.insert(*owner, account.clone()).unwrap();
    }
    info!("migrated {} accounts", v5.accounts.get().len());

    for (owner, owned_files) in v4.owned_files.get() {
        v5.owned_files.create_key(*owner).unwrap();
        for owned_file in owned_files {
            v5.owned_files.insert(*owner, *owned_file).unwrap();
        }
    }
    info!("migrated {} owned_files", v5.owned_files.get().len());

    for (owner, shared_files) in v4.shared_files.get() {
        v5.shared_files.create_key(*owner).unwrap();
        for shared_file in shared_files {
            v5.shared_files.insert(*owner, *shared_file).unwrap();
        }
    }
    info!("migrated {} shared_files", v5.shared_files.get().len());

    for (id, children) in v4.file_children.get() {
        v5.file_children.create_key(*id).unwrap();
        for child in children {
            v5.file_children.insert(*id, *child).unwrap();
        }
    }
    info!("migrated {} children", v5.file_children.get().len());
}
