use crate::billing::billing_model::SubscriptionProfile;
use lockbook_models::feature_flag::FeatureFlags;
use lockbook_models::file_metadata::EncryptedFileMetadata;
use lockbook_models::file_metadata::Owner;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct OneKey;

hmdb::schema! {
    ServerV1 {
        usernames: <String, Owner>,
        accounts: <Owner, Account>,
        owned_files: <Owner, Vec<Uuid>>,
        metas: <Uuid, EncryptedFileMetadata>,
        sizes: <Uuid, u64>,
        google_play_ids: <String, Owner>,
        stripe_ids: <String, Owner>,
        feature_flags: <OneKey, FeatureFlags>
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub billing_info: SubscriptionProfile,
}
