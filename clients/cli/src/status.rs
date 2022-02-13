use lockbook_core::{calculate_work, CalculateWorkError, Error as CoreError};
use lockbook_models::work_unit::WorkUnit;

use crate::error::CliResult;
use crate::utils::{account, config, print_last_successful_sync};
use crate::{err, err_unexpected};

pub fn status() -> CliResult<()> {
    account()?;

    let work = calculate_work(&config()?).map_err(|err| match err {
        CoreError::UiError(CalculateWorkError::NoAccount) => err!(NoAccount),
        CoreError::UiError(CalculateWorkError::CouldNotReachServer) => err!(NetworkIssue),
        CoreError::UiError(CalculateWorkError::ClientUpdateRequired) => err!(UpdateRequired),
        CoreError::Unexpected(msg) => err_unexpected!("{}", msg),
    })?;

    work.work_units.into_iter().for_each(|work_unit| {
        let action = match work_unit {
            WorkUnit::LocalChange { .. } => "pushed",
            WorkUnit::ServerChange { .. } => "pulled",
        };

        println!("{} needs to be {}", work_unit.get_metadata().decrypted_name, action)
    });

    print_last_successful_sync()
}
