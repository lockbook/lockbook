use crate::utils::{
    exit_with, exit_with_offline, exit_with_upgrade_required, get_account_or_exit, get_config,
};
use crate::UNEXPECTED_ERROR;
use lockbook_core::{get_usage, Error as CoreError, GetUsageError};

const BYTE: u64 = 1;
const KILOBYTES: u64 = BYTE * 1000;
const MEGABYTES: u64 = KILOBYTES * 1000;
const GIGABYTES: u64 = MEGABYTES * 1000;
const TERABYTES: u64 = GIGABYTES * 1000;

const KILOBYTES_PLUS_ONE: u64 = KILOBYTES + 1;
const MEGABYTES_PLUS_ONE: u64 = MEGABYTES + 1;
const GIGABYTES_PLUS_ONE: u64 = GIGABYTES + 1;
const TERABYTES_PLUS_ONE: u64 = TERABYTES + 1;

pub fn calculate_usage(exact: bool) {
    get_account_or_exit();

    let usages = get_usage(&get_config()).unwrap_or_else(|err| match err {
        CoreError::UiError(GetUsageError::CouldNotReachServer) => exit_with_offline(),
        CoreError::UiError(GetUsageError::ClientUpdateRequired) => exit_with_upgrade_required(),
        CoreError::UiError(GetUsageError::NoAccount) | CoreError::Unexpected(_) => {
            exit_with(&format!("Unexpected Error: {:?}", err), UNEXPECTED_ERROR)
        }
    });

    let usage_in_bytes: u64 = usages.into_iter().map(|usage| usage.byte_secs).sum();

    if exact {
        println!("{}", usage_in_bytes)
    } else {
        match usage_in_bytes {
            0..=KILOBYTES => println!("{} B", usage_in_bytes),
            KILOBYTES_PLUS_ONE..=MEGABYTES => {
                println!("{:.3} kB", usage_in_bytes as f64 / KILOBYTES as f64)
            }
            MEGABYTES_PLUS_ONE..=GIGABYTES => {
                println!("{:.3} MB", usage_in_bytes as f64 / MEGABYTES as f64)
            }
            GIGABYTES_PLUS_ONE..=TERABYTES => {
                println!("{:.3} GB", usage_in_bytes as f64 / GIGABYTES as f64)
            }
            TERABYTES_PLUS_ONE..=u64::MAX => {
                println!("{:.3} TB", usage_in_bytes as f64 / TERABYTES as f64)
            }
        }
    }
}
