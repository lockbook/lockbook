use lockbook_core::GetUsageError;
use lockbook_core::Core;
use lockbook_core::Error as LbError;

use crate::error::CliError;

pub fn calculate_usage(core: &Core, exact: bool) -> Result<(), CliError> {
    core.get_account()?;

    let usage = core.get_usage()?;
    let uncompressed_usage = core.get_uncompressed_usage()?;

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

impl From<LbError<GetUsageError>> for CliError {
    fn from(e: LbError<GetUsageError>) -> Self {
        match e {
            LbError::UiError(err) => match err {
                GetUsageError::CouldNotReachServer => Self::network_issue(),
                GetUsageError::ClientUpdateRequired => Self::update_required(),
            },
            LbError::Unexpected(msg) => Self::unexpected(msg),
        }
    }
}
