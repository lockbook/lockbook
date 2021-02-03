use std::io;
use std::io::Write;

use lockbook_core::model::work_unit::WorkUnit;
use lockbook_core::service::sync_service::WorkCalculated;
use lockbook_core::{
    calculate_work, execute_work, set_last_synced, CalculateWorkError, Error as CoreError,
    SetLastSyncedError,
};

use crate::error::CliResult;
use crate::utils::{get_account_or_exit, get_config};
use crate::{err, err_unexpected};

pub fn sync() -> CliResult {
    let account = get_account_or_exit();
    let config = get_config();

    let mut work_calculated: WorkCalculated;
    while {
        work_calculated = calculate_work(&config).map_err(|err| match err {
            CoreError::UiError(err) => match err {
                CalculateWorkError::NoAccount => err!(NoAccount),
                CalculateWorkError::CouldNotReachServer => err!(NetworkIssue),
                CalculateWorkError::ClientUpdateRequired => err!(UpdateRequired),
            },
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
        })?;
        !work_calculated.work_units.is_empty()
    } {
        let mut there_were_errors = false;

        for work_unit in work_calculated.work_units {
            let action = match &work_unit {
                WorkUnit::LocalChange { metadata } => format!("Pushing: {}", metadata.name),
                WorkUnit::ServerChange { metadata } => format!("Pulling: {}", metadata.name),
            };

            let _ = io::stdout().flush();
            match execute_work(&config, &account, work_unit) {
                Ok(_) => println!("{:<50}Done.", action),
                Err(error) => {
                    there_were_errors = true;
                    eprintln!("{:<50}{}", action, format!("Skipped: {:?}", error))
                }
            }
        }

        if !there_were_errors {
            set_last_synced(&config, work_calculated.most_recent_update_from_server).map_err(
                |err| match err {
                    CoreError::UiError(SetLastSyncedError::Stub) => err_unexpected!("impossible"),
                    CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
                },
            )?;
        }
    }

    println!("Sync complete.");
    Ok(())
}
