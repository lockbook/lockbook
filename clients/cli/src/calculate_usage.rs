use crate::error::CliResult;
use crate::utils::{get_account_or_exit, get_config};
use crate::{err, err_unexpected};
use lockbook_core::{Error as CoreError, GetUsageError};

pub fn calculate_usage(exact: bool) -> CliResult<()> {
    get_account_or_exit();

    let usage =
        lockbook_core::get_local_and_server_usage(&get_config(), exact).map_err(
            |err| match err {
                CoreError::UiError(GetUsageError::CouldNotReachServer) => err!(NetworkIssue),
                CoreError::UiError(GetUsageError::ClientUpdateRequired) => err!(UpdateRequired),
                CoreError::UiError(GetUsageError::NoAccount) | CoreError::Unexpected(_) => {
                    err_unexpected!("{:?}", err)
                }
            },
        )?;

    println!(
        "Uncompressed File Size: {}",
        usage.readable_metrics.uncompressed_usage
    );
    println!(
        "Server Utilization: {}",
        usage.readable_metrics.server_usage
    );
    println!("Server Data Cap: {}", usage.readable_metrics.data_cap);
    Ok(())
}
