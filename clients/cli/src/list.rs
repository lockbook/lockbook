use crate::utils::{connect_to_db, get_account};
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::service::clock_service::Clock;
use lockbook_core::{DefaultClock, DefaultFileMetadataRepo};

use chrono::Duration;
use chrono_human_duration::ChronoHumanDuration;

pub fn list() {
    let db = connect_to_db();

    get_account(&db);

    DefaultFileMetadataRepo::get_all(&db)
        .expect("Failed to retrieve content from FileMetadataRepo")
        .into_iter()
        .for_each(|metadata| println!("{}: {:?}", metadata.name.trim(), metadata.status));

    let last_updated = DefaultFileMetadataRepo::last_updated(&db)
        .expect("Failed to retrieve content from FileMetadataRepo");

    let duration = if last_updated != 0 {
        let duration =
            Duration::milliseconds((DefaultClock::get_time() as u64 - last_updated) as i64);
        duration.format_human().to_string()
    } else {
        "never".to_string()
    };

    println!("Last updated: {}.", duration);
}
