use lockbook_core::model::work_unit::WorkUnit;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::DefaultSyncService;

use crate::utils::{connect_to_db, get_account, print_last_successful_sync};

pub fn status() {
    get_account(&connect_to_db());

    DefaultSyncService::calculate_work(&connect_to_db())
        .expect("Failed to calculate work required to sync")
        .work_units
        .into_iter()
        .for_each(|work| match work {
            WorkUnit::LocalChange { metadata } => println!("{} needs to be pushed", metadata.name),
            WorkUnit::ServerChange { metadata } => println!("{} needs to be pulled", metadata.name),
        });

    print_last_successful_sync(&connect_to_db());
}
