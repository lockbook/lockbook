use db_rs::{DbResult, List, LookupTable, Single};
use db_rs_derive::Schema;

use lockbook_shared::account::Account;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::signed_file::SignedFile;

use uuid::Uuid;

use crate::service::activity_service::DocEvent;

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

impl CoreV3 {
    pub fn clear(&mut self) -> DbResult<()> {
        self.account.clear()?;
        self.last_synced.clear()?;
        self.root.clear()?;
        self.local_metadata.clear()?;
        self.base_metadata.clear()?;
        self.pub_key_lookup.clear()?;
        // self.doc_events.clear()?; // TODO: update db-rs
        Ok(())
    }
}
