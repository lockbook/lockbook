use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;
use lockbook_core::DefaultFileMetadataRepo;

use crate::utils::{connect_to_db, get_account, print_last_successful_sync};

pub fn list() {
    let db = connect_to_db();

    get_account(&db);

    DefaultFileMetadataRepo::get_all_paths(&db)
        .expect("Failed to retrieve content from FileMetadataRepo")
        .into_iter()
        .for_each(|path| println!("{}", path));

    print_last_successful_sync(&db);
}
