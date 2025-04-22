use db_rs::{List, LookupTable, Single};
use db_rs_derive::Schema;
use uuid::Uuid;

use super::{account::Account, file_metadata::Owner, server_meta::ServerMeta};



#[derive(Schema)]
pub struct AccountV1 {
    pub metas: LookupTable<Uuid, ServerMeta>,
    pub shared_files: List<(Uuid, Owner)>,
    pub sizes: LookupTable<Uuid, u64>,
    pub last_seen: Single<u64>,
}

