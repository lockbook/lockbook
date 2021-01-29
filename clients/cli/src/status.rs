use lockbook_core::model::work_unit::WorkUnit;
use lockbook_core::{calculate_work, CalculateWorkError, Error as CoreError};

use crate::utils::{
    exit_with_offline, exit_with_upgrade_required, get_account_or_exit, get_config,
    print_last_successful_sync,
};
use crate::{err_unexpected, exitlb};

pub fn status() {
    get_account_or_exit();

    match calculate_work(&get_config()) {
        Ok(work) => work.work_units.into_iter().for_each(|work| match work {
            WorkUnit::LocalChange { metadata } => println!("{} needs to be pushed", metadata.name),
            WorkUnit::ServerChange { metadata } => println!("{} needs to be pulled", metadata.name),
        }),
        Err(err) => match err {
            CoreError::UiError(CalculateWorkError::NoAccount) => exitlb!(NoAccount),
            CoreError::UiError(CalculateWorkError::CouldNotReachServer) => exit_with_offline(),
            CoreError::UiError(CalculateWorkError::ClientUpdateRequired) => {
                exit_with_upgrade_required() //TODO
            }
            CoreError::Unexpected(msg) => err_unexpected!("{}", msg).exit(),
        },
    };

    print_last_successful_sync();
}
