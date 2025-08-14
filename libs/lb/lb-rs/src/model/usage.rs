use crate::model::api::GetUsageResponse;
use crate::service::usage::{UsageItemMetric, UsageMetrics};

const BYTE: u64 = 1;
const KILOBYTE: u64 = BYTE * 1000;
const MEGABYTE: u64 = KILOBYTE * 1000;
const GIGABYTE: u64 = MEGABYTE * 1000;
const TERABYTE: u64 = GIGABYTE * 1000;

const KILOBYTE_MINUS_ONE: u64 = KILOBYTE - 1;
const MEGABYTE_MINUS_ONE: u64 = MEGABYTE - 1;
const GIGABYTE_MINUS_ONE: u64 = GIGABYTE - 1;
const TERABYTE_MINUS_ONE: u64 = TERABYTE - 1;

pub fn bytes_to_human(size: u64) -> String {
    let (unit, abbr) = match size {
        0..=KILOBYTE_MINUS_ONE => (BYTE, "B"),
        KILOBYTE..=MEGABYTE_MINUS_ONE => (KILOBYTE, "KB"),
        MEGABYTE..=GIGABYTE_MINUS_ONE => (MEGABYTE, "MB"),
        GIGABYTE..=TERABYTE_MINUS_ONE => (GIGABYTE, "GB"),
        TERABYTE..=u64::MAX => (TERABYTE, "TB"),
    };

    let size_in_unit = size as f64 / unit as f64;
    let dec = f64::trunc(size_in_unit.fract() * 100.0) / 100.0;

    let num = format!("{:.2}", size_in_unit.trunc() + dec)
        .trim_end_matches(['0'])
        .trim_end_matches(['.'])
        .to_owned();

    format!("{num} {abbr}")
}

pub(crate) fn get_usage(server_usage_and_cap: GetUsageResponse) -> UsageMetrics {
    let server_usage = server_usage_and_cap.sum_server_usage();
    let cap = server_usage_and_cap.cap;

    let readable_usage = bytes_to_human(server_usage);
    let readable_cap = bytes_to_human(cap);

    UsageMetrics {
        usages: server_usage_and_cap.usages,
        server_usage: UsageItemMetric { exact: server_usage, readable: readable_usage },
        data_cap: UsageItemMetric { exact: cap, readable: readable_cap },
    }
}
