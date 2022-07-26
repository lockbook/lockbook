use crate::billing::billing_model::SubscriptionProfile;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::server_file::ServerFile;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

hmdb::schema! {
    ServerV1 {
        usernames: <String, Owner>,
        accounts: <Owner, Account>,
        owned_files: <Owner, Vec<Uuid>>,
        metas: <Uuid, ServerFile>,
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
