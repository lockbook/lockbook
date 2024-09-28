pub mod docs;

use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;

use db_rs::{Db, List, LookupTable, Single, TxHandle};
use db_rs_derive::Schema;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tokio::time;

use crate::logic::signed_file::SignedFile;
use crate::model::account::Account;
use crate::model::file_metadata::Owner;
use crate::Lb;

use uuid::Uuid;

use crate::service::activity::DocEvent;

pub(crate) type LbDb = Arc<RwLock<CoreV3>>;
// todo: limit visibility
pub type CoreDb = CoreV3;

#[derive(Schema, Debug)]
#[cfg_attr(feature = "no-network", derive(Clone))]
pub struct CoreV3 {
    pub account: Single<Account>,
    pub last_synced: Single<i64>,
    pub root: Single<Uuid>,
    pub local_metadata: LookupTable<Uuid, SignedFile>,
    pub base_metadata: LookupTable<Uuid, SignedFile>,
    pub pub_key_lookup: LookupTable<Owner, String>,
    pub doc_events: List<DocEvent>,
}

pub(crate) struct LbRO<'a> {
    guard: RwLockReadGuard<'a, CoreDb>,
}

impl<'a> LbRO<'a> {
    pub fn db(&self) -> &CoreDb {
        self.guard.deref()
    }
}

pub struct LbTx<'a> {
    guard: RwLockWriteGuard<'a, CoreDb>,
    tx: TxHandle,
}

impl<'a> LbTx<'a> {
    pub fn db(&mut self) -> &mut CoreDb {
        self.guard.deref_mut()
    }

    pub fn end(self) {
        self.tx.drop_safely().unwrap();
    }
}

impl Lb {
    pub(crate) async fn ro_tx(&self) -> LbRO<'_> {
        // let guard = time::timeout(Duration::from_secs(1), self.db.read())
        //     .await
        //     .unwrap();

        let guard = self.db.read().await;

        LbRO { guard }
    }

    pub async fn begin_tx(&self) -> LbTx<'_> {
        let mut guard = time::timeout(Duration::from_secs(1), self.db.write())
            .await
            .unwrap();
        let tx = guard.begin_transaction().unwrap();

        LbTx { guard, tx }
    }
}
