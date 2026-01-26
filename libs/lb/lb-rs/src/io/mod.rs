//! Members of this model are concerned with the details of IO, generally
//! disk and network. This is the module any on-disk migrations will live
//! and ideas around network, disk, and memory caches will be expressed.
//! Code here should not be platform dependent, and should strive to be
//! suitable for a range of devices: iPhones with flaky networks to servers
//! and workstations with excellent networks.

pub mod docs;
pub mod network;

use crate::model::account::Account;
use crate::model::core_config::Config;
use crate::model::file_like::FileLike;
use crate::model::file_metadata::Owner;
use crate::model::signed_file::SignedFile;
use crate::model::signed_meta::SignedMeta;
use crate::service::activity::DocEvent;
use crate::service::lb_id::LbID;
use crate::{Lb, LbErrKind, LbResult};
use db_rs::{Db, List, LookupTable, Single, TxHandle};
use db_rs_derive::Schema;
use docs::AsyncDocs;
use std::fs;
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
    pub id: Single<LbID>,
    pub account: Single<Account>,
    pub last_synced: Single<i64>,
    pub root: Single<Uuid>,
    pub local_metadata: LookupTable<Uuid, SignedMeta>,
    pub base_metadata: LookupTable<Uuid, SignedMeta>,

    /// map from pub key to username
    pub pub_key_lookup: LookupTable<Owner, String>,

    pub doc_events: List<DocEvent>,
}

pub async fn migrate_and_init(cfg: &Config, docs: &AsyncDocs) -> LbResult<CoreV4> {
    let cfg = db_rs::Config::in_folder(&cfg.writeable_path);

    let mut db =
        CoreDb::init(cfg.clone()).map_err(|err| LbErrKind::Unexpected(format!("{err:#?}")))?;
    let mut old = CoreV3::init(cfg).map_err(|err| LbErrKind::Unexpected(format!("{err:#?}")))?;

    if db.id.get().is_none() {
        db.id.insert(LbID::generate())?;
    }

    // --- migration begins ---
    let tx = db.begin_transaction()?;

    info!("evaluating migration");
    if old.account.get().is_some() && db.account.get().is_none() {
        info!("performing migration");
        if let Some(account) = old.account.get().cloned() {
            db.account.insert(account)?;
        }

        if let Some(last_synced) = old.last_synced.get().copied() {
            db.last_synced.insert(last_synced)?;
        }

        if let Some(root) = old.root.get().copied() {
            db.root.insert(root)?;
        }
        for (id, file) in old.base_metadata.get() {
            let mut meta: SignedMeta = file.clone().into();
            if meta.is_document() {
                if let Some(doc) = docs.maybe_get(*id, file.document_hmac().copied()).await? {
                    meta.timestamped_value
                        .value
                        .set_hmac_and_size(file.document_hmac().copied(), Some(doc.value.len()));
                } else {
                    warn!("local document missing for {id}");
                }
            }
            db.base_metadata.insert(*id, meta)?;
        }

        for (id, file) in old.local_metadata.get() {
            let mut meta: SignedMeta = file.clone().into();
            if meta.is_document() {
                if let Some(doc) = docs.maybe_get(*id, file.document_hmac().copied()).await? {
                    meta.timestamped_value
                        .value
                        .set_hmac_and_size(file.document_hmac().copied(), Some(doc.value.len()));
                } else {
                    warn!("local document missing for {id}");
                }
            }

            db.local_metadata.insert(*id, meta)?;
        }

        for (o, s) in old.pub_key_lookup.get() {
            db.pub_key_lookup.insert(*o, s.clone())?;
        }

        for event in old.doc_events.get() {
            db.doc_events.push(*event)?;
        }
    } else {
        info!("no migration");
    }

    tx.drop_safely()?;
    // --- migration ends ---

    info!("cleaning up");
    old.account.clear()?;
    let old_db = old.config()?.db_location_v2()?;
    let _ = fs::remove_file(old_db);

    Ok(db)
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
