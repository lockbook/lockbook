use std::io;
use std::io::Write;

use lockbook_core::model::work_unit::WorkUnit;
use lockbook_core::service::sync_service::WorkCalculated;
use lockbook_core::{
    calculate_work, execute_work, set_last_synced, CalculateWorkError, Error as CoreError,
    SetLastSyncedError,
};

use crate::utils::{
    exit_with, exit_with_no_account, exit_with_offline, exit_with_upgrade_required,
    get_account_or_exit, get_config,
};
use crate::UNEXPECTED_ERROR;

pub fn sync() {
    let account = get_account_or_exit();
    let config = get_config();

    let mut work_calculated: WorkCalculated;

    let update_last_synced = |time| match set_last_synced(&config, time) {
        Ok(_) => {}
        Err(err) => match err {
            CoreError::UiError(SetLastSyncedError::Stub) => {
                exit_with("Impossible", UNEXPECTED_ERROR)
            }
            CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    };

    while {
        work_calculated = match calculate_work(&config) {
            Ok(work) => work,
            Err(err) => match err {
                CoreError::UiError(err) => match err {
                    CalculateWorkError::NoAccount => exit_with_no_account(),
                    CalculateWorkError::CouldNotReachServer => exit_with_offline(),
                    CalculateWorkError::ClientUpdateRequired => exit_with_upgrade_required(),
                },
                CoreError::Unexpected(msg) => exit_with(&msg, UNEXPECTED_ERROR),
            },
        };
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
            update_last_synced(work_calculated.most_recent_update_from_server);
        }
    }

    update_last_synced(work_calculated.most_recent_update_from_server);
    println!("Sync complete.");
}
