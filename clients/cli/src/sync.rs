use lockbook_core::service::sync_service::SyncProgress;
use lockbook_core::{get_usage, sync_all, Error, GetUsageError, SyncAllError};
use lockbook_models::work_unit::ClientWorkUnit;

use crate::error::CliResult;
use crate::utils::{account, config};
use crate::{err, err_unexpected};
use lockbook_core::model::state::Config;

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
            SyncAllError::OutOfSpace => err!(OutOfSpace),
        },
        Error::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    warn_about_usage_if_needed(&config)?;

    println!("Sync complete.");

    Ok(())
}

fn warn_about_usage_if_needed(config: &Config) -> CliResult<()> {
    let usage = get_usage(&config).map_err(|err| match err {
        Error::UiError(err) => match err {
            GetUsageError::NoAccount => err!(NoAccount),
            GetUsageError::CouldNotReachServer => err!(NetworkIssue),
            GetUsageError::ClientUpdateRequired => err!(UpdateRequired),
        },
        Error::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    let space_used = {
        let space_used = usage.server_usage.exact as f64 / usage.data_cap.exact as f64;
        if space_used > 1f64 {
            1f64
        } else {
            space_used
        }
    };

    let space_left_percent = ((1f64 - space_used) * 100f64) as u8;

    if space_used > 0.85 {
        eprintln!(
            "You are running out of space! {}% of {} left.",
            space_left_percent, usage.data_cap.readable
        );
    }

    Ok(())
}
