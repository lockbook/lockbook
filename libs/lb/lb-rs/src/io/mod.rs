//! Members of this model are concerned with the details of IO, generally
//! disk and network. This is the module any on-disk migrations will live
//! and ideas around network, disk, and memory caches will be expressed.
//! Code here should not be platform dependent, and should strive to be
//! suitable for a range of devices: iPhones with flaky networks to servers
//! and workstations with excellent networks.

pub mod docs;
pub mod legacy;
pub mod migration;
pub mod network;

use crate::Lb;
use crate::model::account::Account;
use crate::model::file_metadata::Owner;
use crate::model::signed_meta::SignedMeta;
use crate::service::activity::DocEvent;
use crate::service::lb_id::LbID;
use db_rs::View;
use db_rs::guard::WriteTx;
use db_rs::views::{
    composite_view::{Composite, Schema},
    hashmap::DbHashMap,
    option::DbOption,
    vec::DbVec,
};
use db_rs_old::hasher::UuidIdentityHasherBuilder;
use db_rs_old::{Config as OldConfig, Db};
use legacy::CoreV4;
use migration::MigrationResult;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;
use web_time::{Duration, Instant};

pub(crate) type LbDb = Arc<RwLock<CoreDb>>;
// todo: limit visibility
pub type CoreDb = Composite<CoreV5>;

#[derive(Default)]
pub struct CoreV5 {
    pub account: DbOption<Account>,
    pub last_synced: DbOption<i64>,
    pub root: DbOption<Uuid>,
    pub local_metadata: DbHashMap<Uuid, SignedMeta, UuidIdentityHasherBuilder>,
    pub base_metadata: DbHashMap<Uuid, SignedMeta, UuidIdentityHasherBuilder>,

    /// map from pub key to username
    pub pub_key_lookup: DbHashMap<Owner, String>,

    pub doc_events: DbVec<DocEvent>,
    pub id: DbOption<LbID>,
    pub pinned_files: DbVec<Uuid>,

    /// Sentinel for `send_debug_info` throttling: the millisecond timestamp of the
    /// most recent panic file we've already uploaded. `None` means we have never
    /// sent debug info; `Some(0)` means we've sent before but no panic file existed.
    pub last_extracted_panic: DbOption<i64>,
}

impl Schema for CoreV5 {
    fn views_mut(&mut self) -> impl AsMut<[&mut dyn View]> {
        let views: [&mut dyn View; 10] = [
            &mut self.account,
            &mut self.last_synced,
            &mut self.root,
            &mut self.local_metadata,
            &mut self.base_metadata,
            &mut self.pub_key_lookup,
            &mut self.doc_events,
            &mut self.id,
            &mut self.pinned_files,
            &mut self.last_extracted_panic,
        ];
        views
    }
}

pub fn init_with_migration(path: &Path) -> MigrationResult<CoreDb> {
    migration::init_with_migration(path, "CoreV5", |dest: &mut CoreV5| {
        if !path.join("CoreV4.db").try_exists()? {
            if path.join("CoreV4").try_exists()? {
                return Err("upgrade the legacy CoreV4 log with the previous app first".into());
            }
            return Ok(());
        }
        let source = CoreV4::init(OldConfig {
            create_db: false,
            create_path: false,
            ..OldConfig::in_folder(path)
        })?;
        if source.incomplete_write()? {
            return Err("refusing to migrate an incomplete CoreV4 log".into());
        }
        if let Some(value) = source.account.get() {
            dest.account.replace(value.clone())?;
        }
        if let Some(value) = source.last_synced.get() {
            dest.last_synced.replace(*value)?;
        }
        if let Some(value) = source.root.get() {
            dest.root.replace(*value)?;
        }
        for (key, value) in source.local_metadata.get() {
            dest.local_metadata.insert(*key, value.clone())?;
        }
        for (key, value) in source.base_metadata.get() {
            dest.base_metadata.insert(*key, value.clone())?;
        }
        for (key, value) in source.pub_key_lookup.get() {
            dest.pub_key_lookup.insert(*key, value.clone())?;
        }
        for value in source.doc_events.get() {
            dest.doc_events.push(*value)?;
        }
        if let Some(value) = source.id.get() {
            dest.id.replace(*value)?;
        }
        for value in source.pinned_files.get() {
            dest.pinned_files.push(*value)?;
        }
        if let Some(value) = source.last_extracted_panic.get() {
            dest.last_extracted_panic.replace(*value)?;
        }
        Ok(())
    })
}

pub struct LbRO<'a> {
    guard: RwLockReadGuard<'a, CoreDb>,
}

impl LbRO<'_> {
    pub fn db(&self) -> &CoreV5 {
        &self.guard.schema
    }
}

pub struct LbTx<'a> {
    guard: RwLockWriteGuard<'a, CoreDb>,
    tx: Option<WriteTx>,
}

impl LbTx<'_> {
    pub fn db(&mut self) -> &mut CoreV5 {
        &mut self.guard.schema
    }

    pub fn end(mut self) {
        self.tx.take().unwrap().end_tx(&mut *self.guard).unwrap();
    }
}

impl Drop for LbTx<'_> {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            if let Err(error) = tx.end_tx(&mut *self.guard) {
                error!(?error, "failed to flush database transaction on drop");
            }
        }
    }
}

impl Lb {
    pub async fn ro_tx(&self) -> LbRO<'_> {
        let start = Instant::now();

        let guard = self.db.read().await;

        if start.elapsed() > Duration::from_millis(100) {
            warn!("readonly transaction lock acquisition took {:?}", start.elapsed());
        }

        LbRO { guard }
    }

    pub async fn begin_tx(&self) -> LbTx<'_> {
        let start = Instant::now();

        let mut guard = self.db.write().await;

        if start.elapsed() > Duration::from_millis(100) {
            warn!("readwrite transaction lock acquisition took {:?}", start.elapsed());
        }

        let tx = guard.write_tx().unwrap();

        LbTx { guard, tx: Some(tx) }
    }
}
