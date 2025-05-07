use db_rs::{List, LookupTable, Single};
use db_rs_derive::Schema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{account::Account, file_metadata::Owner, server_meta::ServerMeta};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerAccount {
    pub username: String,
    pub billing_info: SubscriptionProfile,
}

pub type ServerDb = ServerV4;

#[derive(Schema)]
#[cfg_attr(feature = "no-network", derive(Clone))]
pub struct ServerV4 {
    #[deprecated]
    pub usernames: LookupTable<String, Owner>,
    pub metas: LookupTable<Uuid, ServerFile>,
    pub sizes: LookupTable<Uuid, u64>,
    #[deprecated]
    pub google_play_ids: LookupTable<String, Owner>,
    #[deprecated]
    pub stripe_ids: LookupTable<String, Owner>,
    #[deprecated]
    pub app_store_ids: LookupTable<String, Owner>,
    #[deprecated]
    pub last_seen: LookupTable<Owner, u64>,
    #[deprecated]
    pub accounts: LookupTable<Owner, Account>,
    pub owned_files: LookupSet<Owner, Uuid>,
    pub shared_files: LookupSet<Owner, Uuid>,
    pub file_children: LookupSet<Uuid, Uuid>,
}

// todo: populate this with the full set of users and their billing stuff
// that logic can start using this schema immediately
#[derive(Schema)]
pub struct ServerV5 {
    pub usernames: LookupTable<String, Owner>,
    pub accounts: LookupTable<Owner, Account>,
    pub google_play_ids: LookupTable<String, Owner>,
    pub stripe_ids: LookupTable<String, Owner>,
    pub app_store_ids: LookupTable<String, Owner>,
}
