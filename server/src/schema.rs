pub mod legacy;

use db_rs::View;
use db_rs::views::{
    composite_view::{Composite, Schema},
    hashmap::DbHashMap,
    hashmap_map::DbHashMapMap,
    hashmap_set::DbHashMapSet,
    option::DbOption,
};
use db_rs_old::{Config as OldConfig, Db};
use lb_rs::io::migration::{self, MigrationResult};
use lb_rs::model::file_metadata::{DocumentHmac, Owner};
use lb_rs::model::server_meta::ServerMeta;
use lb_rs::service::debug::DebugInfo;
use lb_rs::service::lb_id::LbID;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

use self::legacy::ServerV5;
use crate::{billing::billing_model::SubscriptionProfile, defense::BandwidthReport};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub billing_info: SubscriptionProfile,
}

pub type ServerDb = Composite<ServerV6>;

#[derive(Default)]
pub struct ServerV6 {
    pub usernames: DbHashMap<String, Owner>,
    pub metas: DbHashMap<Uuid, ServerMeta>,
    pub google_play_ids: DbHashMap<String, Owner>,
    pub stripe_ids: DbHashMap<String, Owner>,
    pub app_store_ids: DbHashMap<String, Owner>,
    pub last_seen: DbHashMap<Owner, u64>,
    pub accounts: DbHashMap<Owner, Account>,
    pub owned_files: DbHashMapSet<Owner, Uuid>,
    pub shared_files: DbHashMapSet<Owner, Uuid>,
    pub file_children: DbHashMapSet<Uuid, Uuid>,
    pub server_egress: DbOption<BandwidthReport>,
    pub egress_by_owner: DbHashMap<Owner, BandwidthReport>,
    pub scheduled_file_cleanups: DbHashMap<(Uuid, DocumentHmac), i64>,
    pub debug_info: DbHashMapMap<Owner, LbID, DebugInfo>,
}

impl Schema for ServerV6 {
    fn views_mut(&mut self) -> impl AsMut<[&mut dyn View]> {
        let views: [&mut dyn View; 14] = [
            &mut self.usernames,
            &mut self.metas,
            &mut self.google_play_ids,
            &mut self.stripe_ids,
            &mut self.app_store_ids,
            &mut self.last_seen,
            &mut self.accounts,
            &mut self.owned_files,
            &mut self.shared_files,
            &mut self.file_children,
            &mut self.server_egress,
            &mut self.egress_by_owner,
            &mut self.scheduled_file_cleanups,
            &mut self.debug_info,
        ];
        views
    }
}

pub fn init_with_migration(path: &Path) -> MigrationResult<ServerDb> {
    migration::init_with_migration(path, "ServerV6", |dest: &mut ServerV6| {
        if !path.join("ServerV5.db").try_exists()? {
            if path.join("ServerV5").try_exists()? {
                return Err("upgrade the legacy ServerV5 log with the previous server first".into());
            }
            return Ok(());
        }
        let source = ServerV5::init(OldConfig {
            create_db: false,
            create_path: false,
            ..OldConfig::in_folder(path)
        })?;
        if source.incomplete_write()? {
            return Err("refusing to migrate an incomplete ServerV5 log".into());
        }
        for (key, value) in source.usernames.get() {
            dest.usernames.insert(key.clone(), *value)?;
        }
        for (key, value) in source.metas.get() {
            dest.metas.insert(*key, value.clone())?;
        }
        for (key, value) in source.google_play_ids.get() {
            dest.google_play_ids.insert(key.clone(), *value)?;
        }
        for (key, value) in source.stripe_ids.get() {
            dest.stripe_ids.insert(key.clone(), *value)?;
        }
        for (key, value) in source.app_store_ids.get() {
            dest.app_store_ids.insert(key.clone(), *value)?;
        }
        for (key, value) in source.last_seen.get() {
            dest.last_seen.insert(*key, *value)?;
        }
        for (key, value) in source.accounts.get() {
            dest.accounts.insert(*key, value.clone())?;
        }
        for (key, values) in source.owned_files.get() {
            dest.owned_files.create_key(*key)?;
            for value in values {
                dest.owned_files.insert(*key, *value)?;
            }
        }
        for (key, values) in source.shared_files.get() {
            dest.shared_files.create_key(*key)?;
            for value in values {
                dest.shared_files.insert(*key, *value)?;
            }
        }
        for (key, values) in source.file_children.get() {
            dest.file_children.create_key(*key)?;
            for value in values {
                dest.file_children.insert(*key, *value)?;
            }
        }
        if let Some(value) = source.server_egress.get() {
            dest.server_egress.replace(value.clone())?;
        }
        for (key, value) in source.egress_by_owner.get() {
            dest.egress_by_owner.insert(*key, value.clone())?;
        }
        for (key, value) in source.scheduled_file_cleanups.get() {
            dest.scheduled_file_cleanups.insert(*key, *value)?;
        }
        for (key, values) in source.debug_info.get() {
            dest.debug_info.create_key(*key)?;
            for (inner_key, value) in values {
                dest.debug_info.insert(*key, *inner_key, value.clone())?;
            }
        }
        Ok(())
    })
}
