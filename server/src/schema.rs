use crate::billing::billing_model::SubscriptionProfile;
use db_rs::{LookupSet, LookupTable};
use db_rs_derive::Schema;
use lb_rs::model::file_metadata::Owner;
use lb_rs::model::server_meta::ServerMeta;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub billing_info: SubscriptionProfile,
}

pub type ServerDb = ServerV5;

#[derive(Schema)]
#[cfg_attr(feature = "no-network", derive(Clone))]
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
    pub bandwidth_egress: LookupTable<Owner, BandwidthReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthReport {}
