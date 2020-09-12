use lockbook_core::model::work_unit::WorkUnit;
use lockbook_core::{calculate_work, CalculateWorkError};

use crate::utils::{
    exit_with, exit_with_no_account, exit_with_offline, exit_with_upgrade_required,
    get_account_or_exit, get_config, print_last_successful_sync,
};
use crate::UNEXPECTED_ERROR;

pub fn status() {
    get_account_or_exit();

    match calculate_work(&get_config()) {
        Ok(work) => work.work_units.into_iter().for_each(|work| match work {
            WorkUnit::LocalChange { metadata } => println!("{} needs to be pushed", metadata.name),
            WorkUnit::ServerChange { metadata } => println!("{} needs to be pulled", metadata.name),
        }),
        Err(err) => match err {
            CalculateWorkError::NoAccount => exit_with_no_account(),
            CalculateWorkError::CouldNotReachServer => exit_with_offline(),
            CalculateWorkError::ClientUpdateRequired => exit_with_upgrade_required(),
            CalculateWorkError::UnexpectedError(msg) => exit_with(&msg, UNEXPECTED_ERROR),
        },
    };

    print_last_successful_sync();
}
