use lockbook_core::model::errors::GetUsageError;
use lockbook_core::Error as LbError;
use lockbook_core::LbCore;

use crate::error::CliError;
use crate::utils::config;

impl From<LbError<GetUsageError>> for CliError {
    fn from(e: LbError<GetUsageError>) -> Self {
        match e {
            LbError::UiError(err) => match err {
                GetUsageError::NoAccount => CliError::no_account(),
                GetUsageError::CouldNotReachServer => CliError::network_issue(),
                GetUsageError::ClientUpdateRequired => CliError::update_required(),
            },
            LbError::Unexpected(msg) => CliError::unexpected(msg),
        }
    }
}

pub fn calculate_usage(core: &LbCore, exact: bool) -> Result<(), CliError> {
    core.get_account()?;

    let usage = lockbook_core::get_usage(&config()?)?;
    let uncompressed_usage = lockbook_core::get_uncompressed_usage(&config()?)?;

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
