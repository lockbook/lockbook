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
use crate::model::signed_file::SignedFile;
use crate::model::signed_meta::SignedMeta;
use crate::service::activity::DocEvent;
use db_rs::{Db, List, LookupTable, Single, TxHandle};
use db_rs_derive::Schema;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;

pub(crate) type LbDb = Arc<RwLock<CoreDb>>;
// todo: limit visibility
pub type CoreDb = CoreV4;

#[derive(Schema, Debug)]
#[cfg_attr(feature = "no-network", derive(Clone))]
pub struct CoreV3 {
    pub account: Single<Account>,
    pub last_synced: Single<i64>,
    pub root: Single<Uuid>,
    pub local_metadata: LookupTable<Uuid, SignedFile>,
    pub base_metadata: LookupTable<Uuid, SignedFile>,

    /// map from pub key to username
    pub pub_key_lookup: LookupTable<Owner, String>,

    pub doc_events: List<DocEvent>,
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
        let start = std::time::Instant::now();

        // let guard = tokio::time::timeout(std::time::Duration::from_secs(1), self.db.read())
        //     .await
        //     .unwrap();

        let guard = self.db.read().await;

        if start.elapsed() > std::time::Duration::from_millis(100) {
            warn!("readonly transaction lock acquisition took {:?}", start.elapsed());
        }

        LbRO { guard }
    }

    pub async fn begin_tx(&self) -> LbTx<'_> {
        let start = std::time::Instant::now();

        // let mut guard = tokio::time::timeout(std::time::Duration::from_secs(1), self.db.write())
        //     .await
        //     .unwrap();

        let mut guard = self.db.write().await;

        if start.elapsed() > std::time::Duration::from_millis(100) {
            warn!("readwrite transaction lock acquisition took {:?}", start.elapsed());
        }

        let tx = guard.begin_transaction().unwrap();

        LbTx { guard, tx }
    }
}
