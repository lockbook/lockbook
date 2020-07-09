use lockbook_core::repo::file_metadata_repo::{FileMetadataRepo, Filter};
use lockbook_core::DefaultFileMetadataRepo;

use crate::utils::{connect_to_db, get_account, print_last_successful_sync};

pub fn list(file_filter: Option<Filter>) {
    get_account(&connect_to_db());

    DefaultFileMetadataRepo::get_all_paths(&connect_to_db(), file_filter)
        .expect("Failed to retrieve content from FileMetadataRepo")
        .into_iter()
        .for_each(|path| println!("{}", path));

    print_last_successful_sync(&connect_to_db());
}
