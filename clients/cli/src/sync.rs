use lockbook_core::service::sync_service::SyncProgress;
use lockbook_core::{sync_all, Error, SyncAllError};

use crate::error::CliResult;
use crate::utils::get_config;
use crate::{err, err_unexpected};
use lockbook_core::model::client_conversion::ClientWorkUnit;

pub fn sync() -> CliResult<()> {
    let config = get_config();
    let closure = |sync_progress: SyncProgress| {
        match sync_progress.current_work_unit {
            ClientWorkUnit::ServerUnknownName(_) => println!("Pulling: New File"),
            ClientWorkUnit::Server(metadata) => println!("Pulling: {}", metadata.name),
            ClientWorkUnit::Local(metadata) => println!("Pushing: {}", metadata.name),
        };
    };

    sync_all(&config, Some(Box::new(closure))).map_err(|err| match err {
        Error::UiError(err) => match err {
            SyncAllError::NoAccount => err!(NoAccount),
            SyncAllError::ClientUpdateRequired => err!(UpdateRequired),
            SyncAllError::CouldNotReachServer => err!(NetworkIssue),
        },
        Error::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    println!("Sync complete.");

    Ok(())
}
