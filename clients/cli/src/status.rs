use lockbook_core::model::work_unit::WorkUnit;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::DefaultSyncService;

use crate::utils::{connect_to_db, get_account, print_last_successful_sync};

pub fn status() {
    let db = connect_to_db();
    get_account(&db);

    DefaultSyncService::calculate_work(&db)
        .expect("Failed to calculate work required to sync")
        .work_units
        .into_iter()
        .for_each(|work| match work {
            WorkUnit::LocalChange { metadata } => println!("{} needs to be pushed", metadata.name),
            WorkUnit::ServerChange { metadata } => println!("{} needs to be pulled", metadata.name),
        });

    print_last_successful_sync(&db);
}
