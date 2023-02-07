use db_rs::{LookupTable, Single};
use db_rs_derive::Schema;

use lockbook_shared::account::Account;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::signed_file::SignedFile;

use uuid::Uuid;

pub mod schema;
pub mod schema_v2;

#[derive(Schema)]
struct CoreV3 {
    account: Single<Account>,
    last_synced: Single<i64>,
    root: Single<Uuid>,
    local_metadata: LookupTable<Uuid, SignedFile>,
    base_metadata: LookupTable<Uuid, SignedFile>,
    pub_key_lookup: LookupTable<String, Owner>,
}
