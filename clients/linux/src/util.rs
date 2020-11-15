pub const BYTE: u64 = 1;
pub const KILOBYTE: u64 = BYTE * 1024;
pub const MEGABYTE: u64 = KILOBYTE * 1024;
pub const GIGABYTE: u64 = MEGABYTE * 1024;
pub const TERABYTE: u64 = GIGABYTE * 1024;

const KILOBYTE_PLUS_ONE: u64 = KILOBYTE + 1;
const MEGABYTE_PLUS_ONE: u64 = MEGABYTE + 1;
const GIGABYTE_PLUS_ONE: u64 = GIGABYTE + 1;
const TERABYTE_PLUS_ONE: u64 = TERABYTE + 1;

pub struct Util;
impl Util {
    pub fn human_readable_bytes(v: u64) -> String {
        let (unit, abbr) = match v {
            0..=KILOBYTE => (BYTE, ""),
            KILOBYTE_PLUS_ONE..=MEGABYTE => (KILOBYTE, "K"),
            MEGABYTE_PLUS_ONE..=GIGABYTE => (MEGABYTE, "M"),
            GIGABYTE_PLUS_ONE..=TERABYTE => (GIGABYTE, "G"),
            TERABYTE_PLUS_ONE..=u64::MAX => (TERABYTE, "T"),
        };
        format!("{:.3} {}B", v as f64 / unit as f64, abbr)
    }
}
