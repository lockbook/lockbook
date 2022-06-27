use lockbook_core::CalculateWorkError;
use lockbook_core::Core;
use lockbook_core::Error as LbError;
use lockbook_models::work_unit::WorkUnit;

use crate::error::CliError;
use crate::utils::print_last_successful_sync;

pub fn status(core: &Core) -> Result<(), CliError> {
    core.get_account()?;

    let work = core.calculate_work().map_err(|err| match err {
        LbError::UiError(err) => match err {
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

    print_last_successful_sync(core)
}
