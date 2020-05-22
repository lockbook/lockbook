use crate::utils::{connect_to_db, get_account};

use lockbook_core::model::work_unit::get_verb;
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::sync_service::SyncService;
use lockbook_core::{DefaultFileMetadataRepo, DefaultSyncService};

pub fn sync() {
    let db = connect_to_db();
    let account = get_account(&db);

    let work_calculated =
        DefaultSyncService::calculate_work(&db).expect("Failed to calculate work required to sync");

    let mut no_errors_during_sync = true;

    work_calculated.work_units.into_iter().for_each(|work| {
        println!("{}", get_verb(&work));
        match DefaultSyncService::execute_work(&db, &account, work) {
            Ok(_) => println!("Done."),
            Err(error) => {
                no_errors_during_sync = false;
                eprintln!("Failed: {:?}", error)
            }
        }
    });

    if no_errors_during_sync {
        DefaultFileMetadataRepo::set_last_updated(
            &db,
            &work_calculated.most_recent_update_from_server,
        )
        .expect("Failed to record successful completion of sync");
    }
}
