use std::io;
use std::io::Write;

use lockbook_core::service::sync_service::{SyncProgress, SyncState};
use lockbook_core::{sync_all, Error, SyncAllError};
use lockbook_models::work_unit::WorkUnit;

use crate::error::CliResult;
use crate::utils::get_config;
use crate::{err, err_unexpected};

pub fn sync() -> CliResult<()> {
    let config = get_config();
    let closure = |sync_progress: SyncProgress| {
        if let SyncState::BeforeStep = sync_progress.state {
            return;
        }

        let action = match &sync_progress.current_work_unit {
            WorkUnit::LocalChange { metadata } => format!("Pushing: {}", metadata.name),
            WorkUnit::ServerChange { metadata } => format!("Pulling: {}", metadata.name),
        };

        let _ = io::stdout().flush();

        match sync_progress.state {
            SyncState::ErrStep => eprintln!("{:<50}{}", action, "Skipped".to_string()),
            SyncState::OkStep => println!("{:<50}Done.", action),
            SyncState::BeforeStep => {}
        }
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
