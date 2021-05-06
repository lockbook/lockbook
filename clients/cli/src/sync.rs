use lockbook_core::service::sync_service::SyncProgress;
use lockbook_core::{sync_all, Error, SyncAllError};
use lockbook_models::work_unit::WorkUnit;

use crate::error::CliResult;
use crate::utils::get_config;
use crate::{err, err_unexpected};

pub fn sync() -> CliResult<()> {
    let config = get_config();
    let closure = |sync_progress: SyncProgress| {
        match &sync_progress.current_work_unit {
            WorkUnit::LocalChange { metadata } => println!("Pushing: {}", metadata.name),
            WorkUnit::ServerChange { metadata } => println!("Pulling: {}", metadata.name),
        };
    };

    sync_all(&config, Some(Box::new(closure))).map_err(|err| match err {
        Error::UiError(err) => match err {
            SyncAllError::NoAccount => err!(NoAccount),
            SyncAllError::ClientUpdateRequired => err!(UpdateRequired),
            SyncAllError::CouldNotReachServer => err!(NetworkIssue),
            SyncAllError::ExecuteWorkError => err!(ExecuteWorkError),
        },
        Error::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    println!("Sync complete.");

    Ok(())
}
