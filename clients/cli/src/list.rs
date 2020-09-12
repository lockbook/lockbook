use lockbook_core::repo::file_metadata_repo::Filter;
use lockbook_core::list_paths;

use crate::utils::{get_config, print_last_successful_sync, prepare_db_and_get_account_or_exit};

pub fn list(file_filter: Option<Filter>) {
    prepare_db_and_get_account_or_exit();

    list_paths(&get_config(), file_filter)
        .expect("Failed to retrieve content from FileMetadataRepo")
        .into_iter()
        .for_each(|path| println!("{}", path));

    print_last_successful_sync();
}
