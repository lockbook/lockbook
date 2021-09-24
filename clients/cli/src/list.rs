use lockbook_core::list_paths;
use lockbook_core::service::path_service::Filter;

use crate::error::CliResult;
use crate::utils::{account, config, print_last_successful_sync};

pub fn list(file_filter: Option<Filter>) -> CliResult<()> {
    account()?;

    list_paths(&config()?, file_filter)
        .expect("Failed to retrieve content from FileMetadataRepo")
        .into_iter()
        .for_each(|path| println!("{}", path));

    print_last_successful_sync()
}
