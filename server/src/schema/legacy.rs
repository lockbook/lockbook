use super::Account;
use crate::defense::BandwidthReport;
use db_rs::{LookupMap, LookupSet, LookupTable, Single};
use db_rs_derive::Schema;
use db_rs_old as db_rs;
use lb_rs::model::file_metadata::{DocumentHmac, Owner};
use lb_rs::model::server_meta::ServerMeta;
use lb_rs::service::debug::DebugInfo;
use lb_rs::service::lb_id::LbID;
use uuid::Uuid;

#[derive(Schema)]
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
    pub server_egress: Single<BandwidthReport>,
    pub egress_by_owner: LookupTable<Owner, BandwidthReport>,
    pub scheduled_file_cleanups: LookupTable<(Uuid, DocumentHmac), i64>,
    pub debug_info: LookupMap<Owner, LbID, DebugInfo>,
}
