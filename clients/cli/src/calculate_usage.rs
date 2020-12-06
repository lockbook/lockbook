use crate::utils::{
    exit_with, exit_with_offline, exit_with_upgrade_required, get_account_or_exit, get_config,
};
use crate::UNEXPECTED_ERROR;
use lockbook_core::{get_usage, Error as CoreError, GetUsageError};

const BYTE: u64 = 1;
const KILOBYTE: u64 = BYTE * 1000;
const MEGABYTE: u64 = KILOBYTE * 1000;
const GIGABYTE: u64 = MEGABYTE * 1000;
const TERABYTE: u64 = GIGABYTE * 1000;

const KILOBYTE_PLUS_ONE: u64 = KILOBYTE + 1;
const MEGABYTE_PLUS_ONE: u64 = MEGABYTE + 1;
const GIGABYTE_PLUS_ONE: u64 = GIGABYTE + 1;
const TERABYTE_PLUS_ONE: u64 = TERABYTE + 1;

pub fn calculate_usage(exact: bool) {
    get_account_or_exit();

    let usage_in_bytes = match get_usage(&get_config()) {
        Ok(usages) => usages.into_iter().map(|usage| usage.byte_secs).sum(),
        Err(err) => match err {
            CoreError::UiError(GetUsageError::CouldNotReachServer) => exit_with_offline(),
            CoreError::UiError(GetUsageError::ClientUpdateRequired) => exit_with_upgrade_required(),
            CoreError::UiError(GetUsageError::NoAccount) | CoreError::Unexpected(_) => {
                exit_with(&format!("Unexpected Error: {:?}", err), UNEXPECTED_ERROR)
            }
        },
    };

    if exact {
        println!("{}", usage_in_bytes)
    } else {
        let (unit, abbr) = match usage_in_bytes {
            0..=KILOBYTE => (BYTE, ""),
            KILOBYTE_PLUS_ONE..=MEGABYTE => (KILOBYTE, "K"),
            MEGABYTE_PLUS_ONE..=GIGABYTE => (MEGABYTE, "M"),
            GIGABYTE_PLUS_ONE..=TERABYTE => (GIGABYTE, "G"),
            TERABYTE_PLUS_ONE..=u64::MAX => (TERABYTE, "T"),
        };
        println!("{:.3} {}B", usage_in_bytes as f64 / unit as f64, abbr)
    }
}
