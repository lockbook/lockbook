use lockbook_core::model::work_unit::get_verb;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::{DefaultFileMetadataRepo, DefaultSyncService};

use crate::utils::{connect_to_db, get_account};

pub fn sync() {
    let db = connect_to_db();
    let account = get_account(&db);

    let mut work_calculated =
        DefaultSyncService::calculate_work(&db).expect("Failed to calculate work required to sync");

    while !work_calculated.work_units.is_empty() {
        for work_unit in work_calculated.work_units {
            print!("{}... ", get_verb(&work_unit));
            match DefaultSyncService::execute_work(&db, &account, work_unit) {
                Ok(_) => println!("Done."),
                Err(error) => eprintln!("Failed: {:?}", error),
            }
        }

        work_calculated = DefaultSyncService::calculate_work(&db)
            .expect("Failed to calculate work required to sync");
    }

    DefaultFileMetadataRepo::set_last_updated(&db, work_calculated.most_recent_update_from_server)
        .expect("Failed to save last updated");

    println!("Sync complete.");
}
