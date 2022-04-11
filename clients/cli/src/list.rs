use lockbook_core::service::path_service::Filter;
use lockbook_core::LbCore;

use crate::error::CliError;
use crate::utils::print_last_successful_sync;

pub fn list(core: &LbCore, file_filter: Option<Filter>) -> Result<(), CliError> {
    core.get_account()?;

    core.list_paths(file_filter)
        .expect("Failed to retrieve content from FileMetadataRepo")
        .into_iter()
        .for_each(|path| println!("{}", path));

    print_last_successful_sync()
}
