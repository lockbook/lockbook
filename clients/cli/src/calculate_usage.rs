use crate::exitlb;
use crate::utils::{
    exit_with_offline, exit_with_upgrade_required, get_account_or_exit, get_config,
};
use lockbook_core::{Error as CoreError, GetUsageError};

pub fn calculate_usage(exact: bool) {
    get_account_or_exit();

    match lockbook_core::get_usage_human_string(&get_config(), exact) {
        Ok(readable_usage) => println!("{}", readable_usage),
        Err(err) => match err {
            CoreError::UiError(GetUsageError::CouldNotReachServer) => exit_with_offline(),
            CoreError::UiError(GetUsageError::ClientUpdateRequired) => exit_with_upgrade_required(),
            CoreError::UiError(GetUsageError::NoAccount) | CoreError::Unexpected(_) => {
                exitlb!(Unexpected, "Unexpected Error: {:?}", err)
            }
        },
    };
}
