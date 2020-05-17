use crate::utils::{connect_to_db, get_account, print_last_successful_sync};
use lockbook_core::repo::file_metadata_repo::FileMetadataRepo;

use lockbook_core::DefaultFileMetadataRepo;

pub fn list() {
    let db = connect_to_db();

    get_account(&db);

    DefaultFileMetadataRepo::get_all(&db)
        .expect("Failed to retrieve content from FileMetadataRepo")
        .into_iter()
        .for_each(|metadata| println!("{}", metadata.file_name.trim()));

    print_last_successful_sync(&db);
}
