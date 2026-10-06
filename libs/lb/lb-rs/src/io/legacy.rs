use crate::model::account::Account;
use crate::model::file_metadata::Owner;
use crate::model::signed_meta::SignedMeta;
use crate::service::activity::DocEvent;
use crate::service::lb_id::LbID;
use db_rs::hasher::UuidIdentityHasherBuilder;
use db_rs::{List, LookupTable, Single};
use db_rs_derive::Schema;
use db_rs_old as db_rs;
use uuid::Uuid;

#[derive(Schema)]
pub struct CoreV4 {
    pub account: Single<Account>,
    pub last_synced: Single<i64>,
    pub root: Single<Uuid>,
    pub local_metadata: LookupTable<Uuid, SignedMeta, UuidIdentityHasherBuilder>,
    pub base_metadata: LookupTable<Uuid, SignedMeta, UuidIdentityHasherBuilder>,
    pub pub_key_lookup: LookupTable<Owner, String>,
    pub doc_events: List<DocEvent>,
    pub id: Single<LbID>,
    pub pinned_files: List<Uuid>,
    pub last_extracted_panic: Single<i64>,
}
