use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::{DefaultFileMetadataRepo, DefaultSyncService};

use crate::utils::{connect_to_db, get_account};
use lockbook_core::model::work_unit::WorkUnit;

pub fn sync() {
    let account = get_account(&connect_to_db());

    let mut work_calculated = DefaultSyncService::calculate_work(&connect_to_db())
        .expect("Failed to calculate work required to sync");

    while !work_calculated.work_units.is_empty() {
        for work_unit in work_calculated.work_units {
            println!(
                "{}",
                match work_unit.clone() {
                    WorkUnit::LocalChange { metadata } =>
                        format!("Syncing: {} to server", metadata.name),
                    WorkUnit::ServerChange { metadata } =>
                        format!("Syncing: {} from server", metadata.name),
                }
            );
            match DefaultSyncService::execute_work(&connect_to_db(), &account, work_unit) {
                Ok(_) => println!("Success."),
                Err(error) => eprintln!("Failed: {:?}", error),
            }
        }

        work_calculated = DefaultSyncService::calculate_work(&connect_to_db())
            .expect("Failed to calculate work required to sync");
    }

    DefaultFileMetadataRepo::set_last_updated(
        &connect_to_db(),
        work_calculated.most_recent_update_from_server,
    )
    .expect("Failed to save last updated");

    println!("Sync complete.");
}
