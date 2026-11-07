//! Members of this model are concerned with the details of IO, generally
//! disk and network. This is the module any on-disk migrations will live
//! and ideas around network, disk, and memory caches will be expressed.
//! Code here should not be platform dependent, and should strive to be
//! suitable for a range of devices: iPhones with flaky networks to servers
//! and workstations with excellent networks.

pub mod docs;
pub mod legacy;
pub mod network;
pub mod server;

use crate::Lb;
use crate::model::account::Account;
use crate::model::file_metadata::Owner;
use crate::model::signed_meta::SignedMeta;
use crate::service::activity::DocEvent;
use crate::service::budget::{LOCK_BUDGET, lock_budget_destroyed};
use crate::service::lb_id::LbID;
#[cfg(all(unix, not(target_os = "ios")))]
use crate::subscribers::ipc::Ipc;
use db_rs::View;
use db_rs::config::Config;
use db_rs::errors::Result as DbResult;
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
use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;
use web_time::{Duration, Instant};

pub(crate) type LbDb = Arc<RwLock<CoreDb>>;
// todo: limit visibility
pub type CoreDb = Composite<CoreV5>;
pub type MigrationResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

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
    let directory = path.join("CoreV5");
    fs::create_dir_all(&directory)?;
    let mut db = CoreDb::init(&Config::default().log_location(directory))?;
    let old_path = path.join("CoreV4.db");

    if db.schema.account.is_none() {
        if !old_path.try_exists()? && !path.join("CoreV4").try_exists()? {
            return Ok(db);
        }
        let source = CoreV4::init(OldConfig {
            create_db: false,
            create_path: false,
            ..OldConfig::in_folder(path)
        })?;
        let tx = db.write_tx()?;
        let dest = &mut db.schema;
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
        tx.end_tx(&mut db)?;
        drop(source);
        fs::remove_file(old_path)?;
    }
    Ok(db)
}

pub struct LbRO<'a> {
    guard: RwLockReadGuard<'a, CoreDb>,
    budget_start: Option<Instant>,
}

impl LbRO<'_> {
    pub fn db(&self) -> &CoreV5 {
        &self.guard.schema
    }
}

pub struct LbTx<'a> {
    guard: RwLockWriteGuard<'a, CoreDb>,
    tx: Option<WriteTx>,
    #[cfg(all(unix, not(target_os = "ios")))]
    ipc: Option<&'a Ipc>,
    budget_start: Option<Instant>,
}

/// Called while the guard is still held, so the panic report is written before any waiter,
/// such as a UI thread about to blow its frame budget, can proceed. The process then exits,
/// whichever thread held the lock, as a frame overrun on the UI thread does.
fn check_hold(lock: &str, budget_start: Option<Instant>) {
    let Some(start) = budget_start else { return };
    let held = start.elapsed();
    if held > LOCK_BUDGET && !std::thread::panicking() {
        let _ = std::panic::catch_unwind(|| lock_budget_destroyed(lock, held, LOCK_BUDGET));
        std::process::exit(101);
    }
}

impl Drop for LbRO<'_> {
    fn drop(&mut self) {
        check_hold("read", self.budget_start);
    }
}

impl LbTx<'_> {
    pub fn db(&mut self) -> &mut CoreV5 {
        &mut self.guard.schema
    }

    pub fn end(mut self) {
        self.flush().unwrap();
    }

    fn flush(&mut self) -> DbResult<()> {
        let tx = self.tx.take().unwrap();
        let seq_no = tx.seq_no;
        let committed_seq = tx.end_tx(&mut *self.guard)?;
        // Empty transactions retain the previous sequence, including after catch-up.
        if committed_seq == seq_no {
            #[cfg(all(unix, not(target_os = "ios")))]
            if let Some(ipc) = self.ipc {
                ipc.notify_peers();
            }
        }
        Ok(())
    }
}

impl Drop for LbTx<'_> {
    fn drop(&mut self) {
        if self.tx.is_some() {
            if let Err(error) = self.flush() {
                error!(?error, "failed to flush database transaction on drop");
            }
        }
        check_hold("write", self.budget_start);
    }
}

impl<S> Lb<S> {
    pub async fn ro_tx(&self) -> LbRO<'_> {
        let start = Instant::now();

        let guard = self.db.read().await;

        if start.elapsed() > Duration::from_millis(100) {
            warn!("readonly transaction lock acquisition took {:?}", start.elapsed());
        }

        LbRO { guard, budget_start: self.budget_start() }
    }

    /// With [crate::model::core_config::Config::lock_budget], dev accounts exit when they release a lock held past
    /// [LOCK_BUDGET].
    fn budget_start(&self) -> Option<Instant> {
        let enforced = self.config.lock_budget && self.get_account().is_ok_and(Account::is_dev);
        enforced.then(Instant::now)
    }

    /// note: you cannot call init for the same config, from the same tokio runtime, or you risk
    /// deadlock. See the test `contended_transactions_do_not_block_the_runtime` for more info.
    /// several workarounds exist for this situation that shouldn't really be required:
    /// Lb is cheap to clone within the same runtime
    /// Different data directories are fine
    ///
    /// If this situation actually becomes required:
    ///
    /// a config could be added to turn off file locks which is what results in tokio deadlocks, but
    /// then what are you doing with multiple log writers writing to the same location?
    ///
    /// we could incur a small perf penalty and put the begin_tx in a tokio::blocking wrapper. This
    /// is how tokio wants us to handle this, but I do not want the overhead.
    pub async fn begin_tx(&self) -> LbTx<'_> {
        let start = Instant::now();

        let mut guard = self.db.write().await;

        if start.elapsed() > Duration::from_millis(100) {
            warn!("readwrite transaction lock acquisition took {:?}", start.elapsed());
        }
        let budget_start = self.budget_start();

        let previous_seq = guard.last_modified();
        let tx = guard.write_tx().unwrap();
        self.notify_catch_up(&guard.schema, previous_seq).await;

        LbTx {
            guard,
            tx: Some(tx),
            #[cfg(all(unix, not(target_os = "ios")))]
            ipc: self.ipc.as_deref(),
            budget_start,
        }
    }
}
