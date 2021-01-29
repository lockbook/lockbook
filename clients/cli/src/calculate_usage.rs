use crate::utils::{get_account_or_exit, get_config};
use crate::{err_unexpected, exitlb};
use lockbook_core::{Error as CoreError, GetUsageError};

pub fn calculate_usage(exact: bool) {
    get_account_or_exit();

    match lockbook_core::get_usage_human_string(&get_config(), exact) {
        Ok(readable_usage) => println!("{}", readable_usage),
        Err(err) => match err {
            CoreError::UiError(GetUsageError::CouldNotReachServer) => exitlb!(NetworkIssue),
            CoreError::UiError(GetUsageError::ClientUpdateRequired) => exitlb!(UpdateRequired),
            CoreError::UiError(GetUsageError::NoAccount) | CoreError::Unexpected(_) => {
                err_unexpected!("{:?}", err).exit()
            }
        },
    };
}
