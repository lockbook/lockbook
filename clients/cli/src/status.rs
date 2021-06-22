use lockbook_core::{calculate_work, CalculateWorkError, Error as CoreError};

use crate::error::CliResult;
use crate::utils::{get_account_or_exit, get_config, print_last_successful_sync};
use crate::{err, err_unexpected};

pub fn status() -> CliResult<()> {
    get_account_or_exit();

    let work = calculate_work(&get_config()).map_err(|err| match err {
        CoreError::UiError(CalculateWorkError::NoAccount) => err!(NoAccount),
        CoreError::UiError(CalculateWorkError::CouldNotReachServer) => err!(NetworkIssue),
        CoreError::UiError(CalculateWorkError::ClientUpdateRequired) => err!(UpdateRequired),
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    work.local_files
        .into_iter()
        .for_each(|metadata| println!("{} needs to be pushed", metadata.name));
    work.server_files
        .into_iter()
        .for_each(|metadata| println!("{} needs to be pulled", metadata.name));

    if work.new_files_count != 0 {
        println!(
            "{} new file{} need{} to be pulled",
            work.new_files_count,
            if work.new_files_count == 1 { "" } else { "s" },
            if work.new_files_count == 1 { "s" } else { "" }
        );
    }

    print_last_successful_sync()
}
