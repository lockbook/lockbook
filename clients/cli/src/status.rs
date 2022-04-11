use lockbook_core::model::errors::CalculateWorkError;
use lockbook_core::LbCore;
use lockbook_core::{calculate_work, Error as LbError};
use lockbook_models::work_unit::WorkUnit;

use crate::error::CliError;
use crate::utils::{config, print_last_successful_sync};

pub fn status(core: &LbCore) -> Result<(), CliError> {
    core.get_account()?;

    let work = calculate_work(&config()?).map_err(|err| match err {
        LbError::UiError(err) => match err {
            CalculateWorkError::NoAccount => CliError::no_account(),
            CalculateWorkError::CouldNotReachServer => CliError::network_issue(),
            CalculateWorkError::ClientUpdateRequired => CliError::update_required(),
        },
        LbError::Unexpected(msg) => CliError::unexpected(msg),
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
