use crate::billing::billing_model::SubscriptionProfile;
use db_rs::{LookupSet, LookupTable, Single};
use db_rs_derive::Schema;
use lb_rs::model::server_file::ServerFile;
use lb_rs::model::{file_metadata::Owner, signed_meta::SignedMeta};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub billing_info: SubscriptionProfile,
}

pub type ServerDb = ServerV4;

#[derive(Schema)]
#[cfg_attr(feature = "no-network", derive(Clone))]
pub struct ServerV4 {
    pub usernames: LookupTable<String, Owner>,
    pub metas: LookupTable<Uuid, ServerFile>,
    pub sizes: LookupTable<Uuid, u64>,
    pub google_play_ids: LookupTable<String, Owner>,
    pub stripe_ids: LookupTable<String, Owner>,
    pub app_store_ids: LookupTable<String, Owner>,
    pub last_seen: LookupTable<Owner, u64>,
    pub accounts: LookupTable<Owner, Account>,
    pub owned_files: LookupSet<Owner, Uuid>,
    pub shared_files: LookupSet<Owner, Uuid>,
    pub file_children: LookupSet<Uuid, Uuid>,
}

#[derive(Schema)]
pub struct ServerV5 {
    pub usernames: LookupTable<String, Owner>,
    pub accounts: LookupTable<Owner, Account>,
    pub google_play_ids: LookupTable<String, Owner>,
    pub stripe_ids: LookupTable<String, Owner>,
    pub app_store_ids: LookupTable<String, Owner>,
}

#[derive(Schema)]
pub struct AccountV1 {
    pub metas: LookupTable<Uuid, SignedMeta>,
    pub sizes: LookupTable<Uuid, u64>,
    pub last_seen: Single<u64>,
}
