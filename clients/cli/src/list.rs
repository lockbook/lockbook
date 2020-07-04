use lockbook_core::repo::file_metadata_repo::{FileMetadataRepo, Filter};
use lockbook_core::DefaultFileMetadataRepo;

use crate::utils::{connect_to_db, get_account, print_last_successful_sync};

pub fn list(file_filter: Option<Filter>) {
    let db = connect_to_db();

    get_account(&db);

    DefaultFileMetadataRepo::get_all_paths(&db, file_filter)
        .expect("Failed to retrieve content from FileMetadataRepo")
        .into_iter()
        .for_each(|path| println!("{}", path));

    print_last_successful_sync(&db);
}
