use lockbook_core::model::work_unit::WorkUnit;
use lockbook_core::{calculate_work, CalculateWorkError, Error as CoreError};

use crate::error::CliResult;
use crate::utils::{get_account_or_exit, get_config, print_last_successful_sync};
use crate::{err, err_unexpected};

pub fn status() -> CliResult<()> {
    get_account_or_exit();

    let work = calculate_work(&get_config()).map_err(|err| match err {
        CoreError::UiError(CalculateWorkError::NoAccount) => err!(NoAccount),
        CoreError::UiError(CalculateWorkError::CouldNotReachServer) => err!(NetworkIssue),
        CoreError::UiError(CalculateWorkError::ClientUpdateRequired) => err!(UpdateRequired),
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    work.work_units.into_iter().for_each(|work| match work {
        WorkUnit::LocalChange { metadata } => println!("{} needs to be pushed", metadata.name),
        WorkUnit::ServerChange { metadata } => println!("{} needs to be pulled", metadata.name),
    });

    print_last_successful_sync()
}
