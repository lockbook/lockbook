use lockbook_core::model::errors::SyncAllError;
use lockbook_core::service::sync_service::SyncProgress;
use lockbook_core::{sync_all, Error};
use lockbook_models::work_unit::ClientWorkUnit;

use crate::error::CliResult;
use crate::utils::{account, config};
use crate::{err, err_unexpected};

pub fn sync() -> CliResult<()> {
    account()?;

    let config = config()?;
    let closure = |sync_progress: SyncProgress| {
        match sync_progress.current_work_unit {
            ClientWorkUnit::PullMetadata => println!("Pulling file tree updates"),
            ClientWorkUnit::PushMetadata => println!("Pushing file tree updates"),
            ClientWorkUnit::PullDocument(name) => println!("Pulling: {}", name),
            ClientWorkUnit::PushDocument(name) => println!("Pushing: {}", name),
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
