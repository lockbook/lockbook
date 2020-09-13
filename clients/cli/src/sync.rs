use lockbook_core::{
    calculate_work, execute_work, set_last_synced, CalculateWorkError, SetLastSyncedError,
};

use crate::utils::{
    exit_with, exit_with_no_account, exit_with_offline, exit_with_upgrade_required,
    get_account_or_exit, get_config,
};
use crate::UNEXPECTED_ERROR;
use lockbook_core::model::work_unit::WorkUnit;
use std::io;
use std::io::Write;

pub fn sync() {
    let account = get_account_or_exit();

    let mut work_calculated = match calculate_work(&get_config()) {
        Ok(work) => work,
        Err(err) => match err {
            CalculateWorkError::NoAccount => exit_with_no_account(),
            CalculateWorkError::CouldNotReachServer => exit_with_offline(),
            CalculateWorkError::ClientUpdateRequired => exit_with_upgrade_required(),
            CalculateWorkError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    };

    while !work_calculated.work_units.is_empty() {
        for work_unit in work_calculated.work_units {
            let action = match work_unit.clone() {
                WorkUnit::LocalChange { metadata } => format!("Pushing: {}", metadata.name),
                WorkUnit::ServerChange { metadata } => format!("Pulling: {}", metadata.name),
            };

            let _ = io::stdout().flush();
            match execute_work(&get_config(), &account, work_unit) {
                Ok(_) => println!("{:<50}Done.", action),
                Err(error) => eprintln!("{:<50}{}", action, format!("Skipped: {:?}", error)),
            }
        }

        work_calculated = match calculate_work(&get_config()) {
            Ok(work) => work,
            Err(err) => match err {
                CalculateWorkError::NoAccount => exit_with_no_account(),
                CalculateWorkError::CouldNotReachServer => exit_with_offline(),
                CalculateWorkError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
                CalculateWorkError::ClientUpdateRequired => exit_with_upgrade_required(),
            },
        };
    }

    match set_last_synced(
        &get_config(),
        work_calculated.most_recent_update_from_server,
    ) {
        Ok(_) => {}
        Err(err) => match err {
            SetLastSyncedError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    }

    println!("Sync complete.");
}
