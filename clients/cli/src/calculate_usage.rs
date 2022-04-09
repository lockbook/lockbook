use crate::error::CliResult;
use crate::utils::{account, config};
use crate::{err, err_unexpected};
use lockbook_core::model::errors::GetUsageError;
use lockbook_core::Error as CoreError;

pub fn calculate_usage(exact: bool) -> CliResult<()> {
    account()?;

    let usage = lockbook_core::get_usage(&config()?).map_err(|err| match err {
        CoreError::UiError(GetUsageError::CouldNotReachServer) => err!(NetworkIssue),
        CoreError::UiError(GetUsageError::ClientUpdateRequired) => err!(UpdateRequired),
        CoreError::UiError(GetUsageError::NoAccount) | CoreError::Unexpected(_) => {
            err_unexpected!("{:?}", err)
        }
    })?;

    let uncompressed_usage =
        lockbook_core::get_uncompressed_usage(&config()?).map_err(|err| match err {
            CoreError::UiError(GetUsageError::CouldNotReachServer) => err!(NetworkIssue),
            CoreError::UiError(GetUsageError::ClientUpdateRequired) => err!(UpdateRequired),
            CoreError::UiError(GetUsageError::NoAccount) | CoreError::Unexpected(_) => {
                err_unexpected!("{:?}", err)
            }
        })?;

    let (uncompressed, server_usage, data_cap) = if exact {
        (
            format!("{} B", uncompressed_usage.exact),
            format!("{} B", usage.server_usage.exact),
            format!("{} B", usage.data_cap.exact),
        )
    } else {
        (uncompressed_usage.readable, usage.server_usage.readable, usage.data_cap.readable)
    };

    println!("Uncompressed File Size: {}", uncompressed);
    println!("Server Utilization: {}", server_usage);
    println!("Server Data Cap: {}", data_cap);
    Ok(())
}
