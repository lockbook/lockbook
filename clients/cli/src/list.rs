use lockbook_core::service::path_service::Filter;
use lockbook_core::Core;

use crate::error::CliError;
use crate::utils::print_last_successful_sync;

pub fn list(core: &Core, file_filter: Option<Filter>) -> Result<(), CliError> {
    core.get_account()?;

    core.list_paths(file_filter)?
        .into_iter()
        .for_each(|path| println!("{}", path));

    print_last_successful_sync(core)
}
