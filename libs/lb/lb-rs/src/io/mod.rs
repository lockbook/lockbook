//! Members of this model are concerned with the details of IO, generally
//! disk and network. This is the module any on-disk migrations will live
//! and ideas around network, disk, and memory caches will be expressed.
//! Code here should not be platform dependent, and should strive to be
//! suitable for a range of devices: iPhones with flaky networks to servers
//! and workstations with excellent networks.

pub mod docs;
pub mod network;

use crate::Lb;
use crate::model::account::Account;
use crate::model::file_metadata::Owner;
use crate::model::signed_meta::SignedMeta;
use crate::service::activity::DocEvent;
use crate::service::lb_id::LbID;
use db_rs::{Db, List, LookupTable, Single, TxHandle};
use db_rs_derive::Schema;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;
use web_time::{Duration, Instant};

pub(crate) type LbDb = Arc<RwLock<CoreDb>>;
// todo: limit visibility
pub type CoreDb = CoreV4;

/// Snapshot of a file's state at the last sync-dir reconciliation.
/// Used for 3-way diff between local disk, lockbook, and the last agreed state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsBaseEntry {
    /// Relative path from the sync root on local disk.
    pub local_path: String,
    /// SHA-256 hash of the file content at last agreement.
    pub content_hash: [u8; 32],
    /// Lockbook `last_modified` timestamp at last agreement.
    pub lb_last_modified: u64,
}

#[derive(Schema, Debug)]
#[cfg_attr(feature = "no-network", derive(Clone))]
pub struct CoreV4 {
    pub account: Single<Account>,
    pub last_synced: Single<i64>,
    pub root: Single<Uuid>,
    pub local_metadata: LookupTable<Uuid, SignedMeta>,
    pub base_metadata: LookupTable<Uuid, SignedMeta>,

    /// map from pub key to username
    pub pub_key_lookup: LookupTable<Owner, String>,

    pub doc_events: List<DocEvent>,
    pub id: Single<LbID>,

    /// sync-dir: last agreed state between local filesystem and lockbook
    pub fs_base: LookupTable<Uuid, FsBaseEntry>,
}

pub struct LbRO<'a> {
    guard: RwLockReadGuard<'a, CoreDb>,
}

impl LbRO<'_> {
    pub fn db(&self) -> &CoreDb {
        self.guard.deref()
    }
}

pub struct LbTx<'a> {
    guard: RwLockWriteGuard<'a, CoreDb>,
    tx: TxHandle,
}

impl LbTx<'_> {
    pub fn db(&mut self) -> &mut CoreDb {
        self.guard.deref_mut()
    }

    pub fn end(self) {
        self.tx.drop_safely().unwrap();
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

        let tx = guard.begin_transaction().unwrap();

        LbTx { guard, tx }
    }
}
