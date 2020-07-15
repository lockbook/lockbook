use lockbook_core::repo::file_metadata_repo::Filter;
use lockbook_core::{get_account, list_paths, GetAccountError};

use crate::utils::{
    connect_to_db, exit_with, exit_with_no_account, get_config, print_last_successful_sync,
};
use crate::UNEXPECTED_ERROR;

pub fn list(file_filter: Option<Filter>) {
    match get_account(&get_config()) {
        Ok(_) => {}
        Err(err) => match err {
            GetAccountError::NoAccount => exit_with_no_account(),
            GetAccountError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }

    list_paths(&get_config(), file_filter)
        .expect("Failed to retrieve content from FileMetadataRepo")
        .into_iter()
        .for_each(|path| println!("{}", path));

    print_last_successful_sync(&connect_to_db());
}
