use crate::billing::billing_model::SubscriptionProfile;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::server_file::ServerFile;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

hmdb::schema! {
    ServerV1 {
        usernames: <String, Owner>,
        accounts: <Owner, Account>,
        owned_files: <Owner, HashSet<Uuid>>,
        shared_files: <Owner, HashSet<Uuid>>,
        metas: <Uuid, ServerFile>,
        file_children: <Uuid, HashSet<Uuid>>,
        sizes: <Uuid, u64>,
        google_play_ids: <String, Owner>,
        stripe_ids: <String, Owner>
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub billing_info: SubscriptionProfile,
}
